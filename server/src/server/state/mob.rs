use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use models::position::Position;
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
    /// Mob is dead, waiting for respawn
    Dead { respawn_at: u128 },
}

impl Default for MobAction {
    fn default() -> Self {
        MobAction::Idle
    }
}

pub struct MobTiming {
    /// Tick when mob can move again (after flinch/damage)
    pub canmove_tick: AtomicU64,
}

impl MobTiming {
    pub fn new() -> Self {
        Self {
            canmove_tick: AtomicU64::new(0),
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

    pub fn is_dead(&self) -> bool {
        matches!(self.action, MobAction::Dead { .. })
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.action, MobAction::Idle)
    }

    /// Check if mob can start a new action (not flinching or dead)
    pub fn can_act(&self) -> bool {
        !self.is_flinching() && !self.is_dead()
    }

    // --- State machine transitions ---

    /// Transition to Flinching state when taking damage
    pub fn transition_to_flinching(&mut self, tick: u128) {
        let until = tick + self.damage_motion as u128;
        self.action = MobAction::Flinching { until };
        self.timing.set_canmove_tick(until);
        // Clear movement queue when flinching
        self.movements.clear();
    }

    /// Transition to Moving state
    pub fn transition_to_moving(&mut self) {
        if self.can_act() {
            self.action = MobAction::Moving;
        }
    }

    /// Transition to Idle state
    pub fn transition_to_idle(&mut self) {
        self.action = MobAction::Idle;
    }

    /// Transition to Dead state
    pub fn transition_to_dead(&mut self, respawn_at: u128) {
        self.action = MobAction::Dead { respawn_at };
        self.movements.clear();
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
