use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use movement::position::Position;
use models::status::StatusSnapshot;

use crate::server::model::map_item::{MapItem, MapItemSnapshot, MapItemType, ToMapItem, ToMapItemSnapshot};
use crate::server::model::movement::{Movable, Movement};

/// Mob action state machine
#[derive(Clone, Debug)]
pub enum MobAction {
    /// Mob is idle, waiting for next action
    Idle,
    /// Mob is moving along a path
    Moving,
    /// Mob is chasing a target
    Chasing { target_id: u32 },
    /// Mob is attacking a target
    Attacking { target_id: u32, last_attack_at: u128 },
    /// Mob is flinching from damage (cannot move)
    Flinching { until: u128 },
    /// Mob is returning to spawn area
    Returning,
}

impl Default for MobAction {
    fn default() -> Self {
        MobAction::Idle
    }
}

pub struct MobTiming {
    /// Tick when mob can move again (after flinch/damage)
    pub canmove_tick: AtomicU64,
    /// Tick when mob can attack again (after attack animation)
    pub canattack_tick: AtomicU64,
}

impl MobTiming {
    pub fn new() -> Self {
        Self {
            canmove_tick: AtomicU64::new(0),
            canattack_tick: AtomicU64::new(0),
        }
    }

    /// Set canmove_tick (called when mob takes damage)
    pub fn set_canmove_tick(&self, tick: u128) {
        self.canmove_tick.store(tick as u64, Ordering::Release);
    }

    /// Get canmove_tick (called by mob movement thread)
    pub fn get_canmove_tick(&self) -> u128 {
        self.canmove_tick.load(Ordering::Acquire) as u128
    }

    /// Set canattack_tick (called after mob attacks)
    pub fn set_canattack_tick(&self, tick: u128) {
        self.canattack_tick.store(tick as u64, Ordering::Release);
    }

    /// Get canattack_tick (called by AI to check attack cooldown)
    pub fn get_canattack_tick(&self) -> u128 {
        self.canattack_tick.load(Ordering::Acquire) as u128
    }
}

impl Default for MobTiming {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MobTiming {
    fn clone(&self) -> Self {
        Self {
            canmove_tick: AtomicU64::new(self.canmove_tick.load(Ordering::Relaxed)),
            canattack_tick: AtomicU64::new(self.canattack_tick.load(Ordering::Relaxed)),
        }
    }
}

#[derive(Setters, Clone)]
pub struct Mob {
    pub id: u32,
    pub name: String,
    pub name_english: String,
    pub mob_id: i16,
    pub spawn_id: u32,
    pub status: StatusSnapshot,
    #[set]
    pub x: u16,
    #[set]
    pub y: u16,
    pub map_view: Vec<MapItem>,
    pub is_view_char: bool,
    pub movements: Vec<Movement>,
    pub damages: HashMap<u32, u32>,
    pub last_attacked_at: u128,
    pub to_remove: bool,
    pub last_moved_at: u128,
    pub damage_motion: u32,
    pub timing: MobTiming,
    pub action: MobAction,
    /// AI behavior mode flags (see MobMode enum)
    pub mode: u32,
    /// Attack range (range1 from database)
    pub attack_range: u16,
    /// Chase range (range3 from database)
    pub chase_range: u16,
    /// Attack delay in ms (time between attacks)
    pub atk_delay: u32,
    /// Attack motion duration in ms
    pub atk_motion: u32,
    /// Minimum attack damage
    pub atk1: u16,
    /// Maximum attack damage
    pub atk2: u16,
    /// Current target for passive mobs (set when attacked)
    pub target_id: Option<u32>,
}

pub struct MobMovement {
    pub id: u32,
    pub from: Position,
    pub to: Position,
}

impl Movable for Mob {
    fn movements_mut(&mut self) -> &mut Vec<Movement> {
        &mut self.movements
    }

    fn movements(&self) -> &Vec<Movement> {
        &self.movements
    }

    fn set_movement(&mut self, movements: Vec<Movement>) {
        self.movements = movements;
    }
}

impl Mob {
    pub fn new(
        id: u32,
        x: u16,
        y: u16,
        mob_id: i16,
        spawn_id: u32,
        name: String,
        name_english: String,
        damage_motion: u32,
        status: StatusSnapshot,
        mode: u32,
        attack_range: u16,
        chase_range: u16,
        atk_delay: u32,
        atk_motion: u32,
        atk1: u16,
        atk2: u16,
    ) -> Mob {
        Mob {
            id,
            x,
            y,
            mob_id,
            spawn_id,
            status,
            name,
            name_english,
            map_view: vec![],
            is_view_char: false,
            movements: vec![],
            damages: Default::default(),
            last_attacked_at: 0,
            to_remove: false,
            last_moved_at: 0,
            damage_motion,
            timing: MobTiming::new(),
            action: MobAction::Idle,
            mode,
            attack_range,
            chase_range,
            atk_delay,
            atk_motion,
            atk1,
            atk2,
            target_id: None,
        }
    }

    #[inline]
    pub fn x(&self) -> u16 {
        self.x
    }

    #[inline]
    pub fn y(&self) -> u16 {
        self.y
    }

    pub fn update_map_view(&mut self, map_items: Vec<MapItem>) {
        self.is_view_char = !map_items.is_empty();
        self.map_view = map_items;
    }

    pub fn update_position(&mut self, x: u16, y: u16) {
        #[cfg(feature = "debug_mob_movement")]
        {
            if crate::server::model::path::manhattan_distance(self.x, self.y, x, y) > 2 {
                error!("mob teleported old ({},{}) new ({},{})", self.x, self.y, x, y);
            }
        }
        self.x = x;
        self.y = y;
    }

    pub fn add_attack(&mut self, attacker_id: u32, damage: u32) {
        if damage == 0 {
            return;
        }
        let hp = self.status.hp();
        if damage > hp {
            self.set_hp(0);
        } else {
            self.set_hp(hp - damage);
        }

        let entry = self.damages.entry(attacker_id).or_insert(0);
        *entry += damage;
    }

    pub fn should_die(&self) -> bool {
        self.status.hp() == 0
    }

    pub fn set_hp(&mut self, hp: u32) {
        self.status.set_hp(hp);
    }

    pub fn hp(&self) -> u32 {
        self.status.hp()
    }

    pub fn set_to_remove(&mut self) {
        self.to_remove = true;
    }

    pub fn is_present(&self) -> bool {
        !self.to_remove
    }

    pub fn attacker_with_higher_damage(&self) -> u32 {
        let mut higher_damage: u32 = 0;
        let mut attacker_with_higher_damage = 0;
        for (attacker_id, damage) in self.damages.iter() {
            if *damage > higher_damage {
                attacker_with_higher_damage = *attacker_id;
                higher_damage = *damage;
            }
        }
        attacker_with_higher_damage
    }

    pub fn set_last_moved_at(&mut self, tick: u128) {
        self.last_moved_at = tick;
    }

    /// Check if mob can move at the given tick (atomic check for movement thread)
    pub fn can_move(&self, tick: u128) -> bool {
        tick >= self.timing.get_canmove_tick()
    }

    // --- State machine queries ---

    pub fn is_flinching(&self) -> bool {
        matches!(self.action, MobAction::Flinching { .. })
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.action, MobAction::Idle)
    }

    /// Check if mob can start a new action (not flinching)
    pub fn can_act(&self) -> bool {
        !self.is_flinching()
    }

    // --- State machine transitions ---

    /// Flinching interrupts any action
    pub fn transition_to_flinching(&mut self, tick: u128) {
        let until = tick + self.damage_motion as u128;
        self.action = MobAction::Flinching { until };
        self.timing.set_canmove_tick(until);
        self.movements.clear();
    }

    /// Can transition to Moving from: Idle only
    pub fn transition_to_moving(&mut self) -> bool {
        if matches!(self.action, MobAction::Idle) {
            self.action = MobAction::Moving;
            true
        } else {
            false
        }
    }

    /// Transition to Idle - from Moving, Flinching (after timeout)
    pub fn transition_to_idle(&mut self) -> bool {
        match self.action {
            MobAction::Moving | MobAction::Flinching { .. } => {
                self.action = MobAction::Idle;
                true
            }
            _ => false,
        }
    }

    /// Update flinch state - call each tick to check if flinch is done
    pub fn update_flinch(&mut self, tick: u128) {
        if let MobAction::Flinching { until } = self.action {
            if tick >= until {
                self.action = MobAction::Idle;
            }
        }
    }

    /// Update movement state - call when movement completes
    pub fn update_movement_complete(&mut self) {
        if matches!(self.action, MobAction::Moving) && !self.is_moving() {
            self.action = MobAction::Idle;
        }
    }

    /// Transition to Chasing from Idle or Moving
    /// Transition to Chasing from Idle, Moving, or Attacking (when target moves away)
    pub fn transition_to_chasing(&mut self, target_id: u32) -> bool {
        match self.action {
            MobAction::Idle | MobAction::Moving | MobAction::Attacking { .. } => {
                self.action = MobAction::Chasing { target_id };
                self.movements.clear();
                true
            }
            _ => false,
        }
    }

    /// Transition to Attacking from Idle or Chasing
    pub fn transition_to_attacking(&mut self, target_id: u32, tick: u128) -> bool {
        match self.action {
            MobAction::Idle | MobAction::Chasing { .. } => {
                self.action = MobAction::Attacking {
                    target_id,
                    last_attack_at: tick,
                };
                self.movements.clear();
                true
            }
            _ => false,
        }
    }

    /// Check if mob can attack at the given tick
    pub fn can_attack_at(&self, tick: u128) -> bool {
        tick >= self.timing.get_canattack_tick()
    }

    /// Update last attack time
    pub fn update_last_attack(&mut self, tick: u128) {
        if let MobAction::Attacking { last_attack_at, .. } = &mut self.action {
            *last_attack_at = tick;
        }
    }

    /// Get current target from Chasing/Attacking state or stored target_id
    pub fn get_target_id(&self) -> Option<u32> {
        match &self.action {
            MobAction::Chasing { target_id } => Some(*target_id),
            MobAction::Attacking { target_id, .. } => Some(*target_id),
            _ => self.target_id,
        }
    }

    /// Lose target - return to Idle
    pub fn lose_target(&mut self) {
        self.target_id = None;
        match self.action {
            MobAction::Chasing { .. } | MobAction::Attacking { .. } => {
                self.action = MobAction::Idle;
            }
            _ => {}
        }
    }

    /// Transition to flinching and store attacker as target for passive mobs
    pub fn transition_to_flinching_with_attacker(&mut self, attacker_id: u32, tick: u128) {
        if self.target_id.is_none() {
            self.target_id = Some(attacker_id);
        }
        self.transition_to_flinching(tick);
    }

    /// Check if mob is currently chasing
    pub fn is_chasing(&self) -> bool {
        matches!(self.action, MobAction::Chasing { .. })
    }

    /// Check if mob is currently attacking
    pub fn is_attacking(&self) -> bool {
        matches!(self.action, MobAction::Attacking { .. })
    }

    pub fn position(&self) -> Position {
        Position {
            x: self.x,
            y: self.y,
            dir: 0,
        }
    }
}

impl ToMapItem for Mob {
    fn to_map_item(&self) -> MapItem {
        MapItem::new(self.id, self.mob_id, MapItemType::Mob)
    }
}

impl ToMapItemSnapshot for Mob {
    fn to_map_item_snapshot(&self) -> MapItemSnapshot {
        MapItemSnapshot {
            map_item: self.to_map_item(),
            position: Position {
                x: self.x,
                y: self.y,
                dir: 3, // TODO
            },
        }
    }
}
