use std::sync::mpsc::SyncSender;

use models::enums::mob::MobMode;
use models::position::Position;

use crate::server::model::events::client_notification::Notification;
use crate::server::model::events::map_event::MobAttackCharacter;
use crate::server::model::map::Map;
use crate::server::model::map_item::{MapItemSnapshot, MapItemType};
use crate::server::model::movement::{Movable, Movement};
use crate::server::model::path::{manhattan_distance, path_search_client_side_algorithm};
use crate::server::service::global_config_service::GlobalConfigService;
use crate::server::state::mob::{Mob, MobAction, MobMovement};

pub enum MobAIAction {
    Move(MobMovement),
    Attack(MobAttackCharacter),
}

#[allow(dead_code)]
pub struct MobService {
    client_notification_sender: SyncSender<Notification>,
    configuration_service: &'static GlobalConfigService,
}

impl MobService {
    pub(crate) fn new(client_notification_sender: SyncSender<Notification>, configuration_service: &'static GlobalConfigService) -> Self {
        MobService {
            client_notification_sender,
            configuration_service,
        }
    }

    pub fn action_move(&self, mob: &mut Mob, cells: &[u16], x_size: u16, y_size: u16, start_at: u128) -> Option<MobMovement> {
        // Check state machine - can't move if flinching or dead
        if !mob.can_act() {
            return None;
        }
        if !mob.is_present()
            || mob.is_moving()
            || mob.status.speed() == 1000
            || start_at < mob.last_moved_at
            || start_at - mob.last_moved_at < 500
        {
            return None;
        }
        let mut rng = fastrand::Rng::new();
        let mut movement: Option<MobMovement> = None;
        let rand = rng.i32(0..=100);
        let should_move = if mob.is_view_char {
            rand <= (self.configuration_service.config().game.mob_move_frequency_when_player_around * 100.0) as i32
        } else {
            rand <= (self.configuration_service.config().game.mob_move_frequency_when_no_player_around * 100.0) as i32
        };

        if should_move {
            let rand_distance = rng.usize(2..=8);
            let current_x = mob.x;
            let current_y = mob.y;
            if let Some((x, y)) = Map::find_random_walkable_cell_in_max_range(cells, x_size, y_size, current_x, current_y, rand_distance) {
                if current_x == x && current_y == y {
                    return None;
                }
                // Try to transition first - if blocked, don't set movement
                if !mob.transition_to_moving() {
                    return None;
                }
                let from = Position {
                    x: current_x,
                    y: current_y,
                    dir: 0,
                };
                let to = Position { x, y, dir: 0 };
                movement = Some(MobMovement { id: mob.id, from, to });
                let path = path_search_client_side_algorithm(x_size, y_size, cells, mob.x, mob.y, to.x, to.y);
                let path = Movement::from_path(path, start_at);
                mob.movements = path;
            }
        }
        movement
    }

    /// Main AI decision function - called by map instance loop for each mob
    pub fn action_ai(
        &self,
        mob: &mut Mob,
        characters: &[MapItemSnapshot],
        cells: &[u16],
        x_size: u16,
        y_size: u16,
        tick: u128,
    ) -> Option<MobAIAction> {
        if !mob.is_present() {
            return None;
        }

        // Update flinch first - may transition to Idle if flinch duration ended
        mob.update_flinch(tick);

        if !mob.can_act() {
            return None;
        }

        match &mob.action {
            MobAction::Idle => self.ai_idle(mob, characters, cells, x_size, y_size, tick),
            MobAction::Moving => self.ai_moving(mob, characters, tick),
            MobAction::Chasing { target_id } => {
                self.ai_chasing(mob, *target_id, characters, cells, x_size, y_size, tick)
            }
            MobAction::Attacking { target_id, last_attack_at } => {
                self.ai_attacking(mob, *target_id, *last_attack_at, characters, tick)
            }
            MobAction::Flinching { .. } => None,
            MobAction::Returning => None,
        }
    }

    fn ai_idle(
        &self,
        mob: &mut Mob,
        characters: &[MapItemSnapshot],
        cells: &[u16],
        x_size: u16,
        y_size: u16,
        tick: u128,
    ) -> Option<MobAIAction> {
        // Check if passive mob was attacked and has a target
        if let Some(target_id) = mob.target_id {
            if let Some(target) = self.find_character_by_id(characters, target_id) {
                let distance = manhattan_distance(mob.x, mob.y, target.position.x, target.position.y);
                if distance <= mob.attack_range {
                    mob.transition_to_attacking(target_id, tick);
                    return self.execute_attack(mob, target_id, target.position.x, target.position.y, tick);
                } else if distance <= mob.chase_range {
                    mob.transition_to_chasing(target_id);
                    return self.start_chase(mob, target.position, cells, x_size, y_size, tick);
                }
            } else {
                mob.target_id = None;
            }
        }

        // Aggressive behavior - find nearest player
        let is_aggressive = MobMode::is_aggressive(mob.mode);
        let can_attack = MobMode::can_attack(mob.mode);
        if is_aggressive && can_attack {
            if let Some(target) = self.find_nearest_target(mob, characters) {
                let target_id = target.map_item.id();
                let distance = manhattan_distance(mob.x, mob.y, target.position.x, target.position.y);
                // debug!(
                //     "Aggressive mob {} (mode={}) found target {} at distance {}, attack_range={}, chase_range={}",
                //     mob.name, mob.mode, target_id, distance, mob.attack_range, mob.chase_range
                // );
                if distance <= mob.attack_range {
                    mob.transition_to_attacking(target_id, tick);
                    return self.execute_attack(mob, target_id, target.position.x, target.position.y, tick);
                } else if distance <= mob.chase_range {
                    mob.transition_to_chasing(target_id);
                    return self.start_chase(mob, target.position, cells, x_size, y_size, tick);
                }
            }
        }

        // Random movement if no targets
        self.action_move(mob, cells, x_size, y_size, tick)
            .map(MobAIAction::Move)
    }

    fn ai_moving(
        &self,
        mob: &mut Mob,
        characters: &[MapItemSnapshot],
        tick: u128,
    ) -> Option<MobAIAction> {
        // Check if passive mob was attacked while moving
        if let Some(target_id) = mob.target_id {
            if self.find_character_by_id(characters, target_id).is_some() {
                mob.transition_to_chasing(target_id);
            } else {
                mob.target_id = None;
            }
        }

        // Aggressive mobs can switch to chasing while moving
        if MobMode::is_aggressive(mob.mode) && MobMode::can_attack(mob.mode) {
            if let Some(target) = self.find_nearest_target(mob, characters) {
                let distance = manhattan_distance(mob.x, mob.y, target.position.x, target.position.y);
                if distance <= mob.chase_range {
                    mob.transition_to_chasing(target.map_item.id());
                }
            }
        }

        // Update movement complete state
        mob.update_movement_complete();
        None
    }

    fn ai_chasing(
        &self,
        mob: &mut Mob,
        target_id: u32,
        characters: &[MapItemSnapshot],
        cells: &[u16],
        x_size: u16,
        y_size: u16,
        tick: u128,
    ) -> Option<MobAIAction> {
        let target = self.find_character_by_id(characters, target_id);

        if let Some(target) = target {
            let distance = manhattan_distance(mob.x, mob.y, target.position.x, target.position.y);

            // Target in attack range
            if distance <= mob.attack_range {
                mob.transition_to_attacking(target_id, tick);
                return self.execute_attack(mob, target_id, target.position.x, target.position.y, tick);
            }

            // Target escaped chase range
            if distance > mob.chase_range {
                mob.lose_target();
                return None;
            }

            // Continue chasing - update path if not moving
            if !mob.is_moving() {
                return self.start_chase(mob, target.position, cells, x_size, y_size, tick);
            }
        } else {
            mob.lose_target();
        }

        None
    }

    fn ai_attacking(
        &self,
        mob: &mut Mob,
        target_id: u32,
        last_attack_at: u128,
        characters: &[MapItemSnapshot],
        tick: u128,
    ) -> Option<MobAIAction> {
        let target = self.find_character_by_id(characters, target_id);

        if let Some(target) = target {
            let distance = manhattan_distance(mob.x, mob.y, target.position.x, target.position.y);

            // Target moved out of attack range
            if distance > mob.attack_range {
                mob.transition_to_chasing(target_id);
                return None;
            }

            // Check attack cooldown
            if !mob.can_attack_at(tick) || tick < last_attack_at + mob.atk_delay as u128 {
                return None;
            }

            return self.execute_attack(mob, target_id, target.position.x, target.position.y, tick);
        } else {
            mob.lose_target();
        }

        None
    }

    fn execute_attack(
        &self,
        mob: &mut Mob,
        target_id: u32,
        _target_x: u16,
        _target_y: u16,
        tick: u128,
    ) -> Option<MobAIAction> {
        mob.update_last_attack(tick);
        mob.timing.set_canattack_tick(tick + mob.atk_delay as u128);

        let damage = self.calculate_mob_attack_damage(mob);

        Some(MobAIAction::Attack(MobAttackCharacter {
            mob_id: mob.id,
            target_char_id: target_id,
            damage,
            attack_motion: mob.atk_motion,
            mob_x: mob.x,
            mob_y: mob.y,
        }))
    }

    fn find_character_by_id<'a>(
        &self,
        characters: &'a [MapItemSnapshot],
        char_id: u32,
    ) -> Option<&'a MapItemSnapshot> {
        characters.iter().find(|c| c.map_item.id() == char_id)
    }

    fn find_nearest_target<'a>(
        &self,
        mob: &Mob,
        characters: &'a [MapItemSnapshot],
    ) -> Option<&'a MapItemSnapshot> {
        let mut nearest: Option<(&MapItemSnapshot, u16)> = None;

        for character in characters
            .iter()
            .filter(|c| matches!(c.map_item.object_type(), MapItemType::Character))
        {
            let distance = manhattan_distance(mob.x, mob.y, character.position.x, character.position.y);

            if distance > mob.chase_range {
                continue;
            }

            match nearest {
                None => nearest = Some((character, distance)),
                Some((_, current_dist)) if distance < current_dist => {
                    nearest = Some((character, distance))
                }
                _ => {}
            }
        }

        nearest.map(|(c, _)| c)
    }

    fn start_chase(
        &self,
        mob: &mut Mob,
        target_pos: Position,
        cells: &[u16],
        x_size: u16,
        y_size: u16,
        tick: u128,
    ) -> Option<MobAIAction> {
        let path = path_search_client_side_algorithm(x_size, y_size, cells, mob.x, mob.y, target_pos.x, target_pos.y);

        if path.is_empty() {
            return None;
        }

        let from = mob.position();
        let to = target_pos;
        let path = Movement::from_path(path, tick);
        mob.movements = path;

        Some(MobAIAction::Move(MobMovement { id: mob.id, from, to }))
    }

    fn calculate_mob_attack_damage(&self, mob: &Mob) -> u32 {
        let mut rng = fastrand::Rng::new();
        if mob.atk1 >= mob.atk2 {
            mob.atk1 as u32
        } else {
            rng.u32(mob.atk1 as u32..=mob.atk2 as u32)
        }
    }
}
