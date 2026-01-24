# State Machine Refactor Plan

## The Problem

Current state is a mess - scattered across multiple optional fields:
- `attack: Option<Attack>`
- `skill_in_use: Option<SkillInUse>`
- `movements: Vec<Movement>`
- `sit: bool`

Nothing prevents invalid combinations. And the threading model causes race conditions because movement thread (16ms) and game loop (40ms) both touch character state without proper sync.

Also found out from rathena research:
- Don't send `PacketZcStopmove` for mob flinch - just use `canmove_tick`
- Attack tick should be initialized to current tick, not 0
- The `-40` in movement delay formula is wrong

## Solution

Two parts:
1. Single action enum instead of scattered optionals
2. Atomic timestamps for cross-thread timing checks

## Character Action Enum

```rust
pub enum CharacterAction {
    Idle,

    Moving {
        destination: Position,
        path: Vec<Movement>,
        started_at: u128,
    },

    Attacking {
        target_id: u32,
        repeat: bool,
        started_at: u128,
        last_attack_at: u128,
        attack_motion: u32,
    },

    UsingSkill {
        skill: Box<dyn Skill>,
        target: Option<u32>,
        cast_start: u128,
        cast_end: u128,
    },

    Sitting,

    Dead,
}
```

Now it's impossible to be attacking while sitting, etc.

## Atomic Timing

```mermaid
sequenceDiagram
    participant C as Client
    participant RH as Request Handler
    participant GL as Game Loop (40ms)
    participant ML as Movement Loop (16ms)
    participant AT as Atomic Timing

    C->>RH: Attack Request
    RH->>GL: GameEvent::CharacterAttack
    GL->>GL: Process attack
    GL->>AT: canmove_tick = tick + attack_motion

    C->>RH: Move Request
    RH->>AT: load canmove_tick
    AT-->>RH: 1500 (can't move until then)
    RH->>ML: CharacterMovement(start_at=1500)

    loop Every 16ms
        ML->>AT: load canmove_tick
        AT-->>ML: 1500
        alt tick < canmove_tick
            ML->>ML: skip movement
        else tick >= canmove_tick
            ML->>ML: process movement
        end
    end
```

For cross-thread checks without locks:

```rust
pub struct CharacterTiming {
    pub canmove_tick: AtomicU64,
    pub canact_tick: AtomicU64,
    pub x: AtomicU16,
    pub y: AtomicU16,
}
```

Game loop writes these when state changes. Movement thread reads them to check if movement is allowed.

```rust
// Game loop sets timing when attack starts
character.timing.canmove_tick.store(tick + attack_motion, Ordering::Release);

// Movement thread checks before processing
if tick < character.timing.canmove_tick.load(Ordering::Acquire) {
    return; // can't move yet
}
```

No locks, no sleep, no race conditions.

## Mob Action Enum

```rust
pub enum MobAction {
    Idle { idle_until: u128 },
    Moving { destination: Position, path: Vec<Movement>, started_at: u128 },
    Chasing { target_id: u32, last_path_calc: u128 },
    Attacking { target_id: u32, last_attack_at: u128, attack_motion: u32 },
    Flinching { until: u128 },
    Returning { spawn_point: Position },
    Dead { death_time: u128, respawn_at: u128 },
}
```

Key insight: `Flinching` state + atomic `canmove_tick` replaces the need for `PacketZcStopmove`. Movement thread just skips when `canmove_tick` hasn't passed.

## Changes to Make

### 1. Fix attack tick init (quick win)

In `game_loop.rs:152`:
```rust
// OLD
character.set_attack(target_id, repeat, 0);

// NEW
character.set_attack(target_id, repeat, tick);
```

### 2. Remove sleep from movement handler

In `movement.rs`, currently we do:
```rust
if attack_still_in_progress {
    sleep(Duration::from_millis(delay)); // blocks thread!
}
```

Change to:
```rust
let start_at = max(tick, character.timing.canmove_tick.load(Ordering::Acquire));
// pass start_at to movement event, no sleep
```

Movement loop respects `start_at` - movement doesn't begin until then.

### 3. Fix movement delay formula

In `movement.rs:57`:
```rust
// OLD - arbitrary -40 subtraction
if attack.last_attack_tick + attack.last_attack_motion - 40 > start_at

// NEW - use atomic canmove_tick, no subtraction
if tick < character.timing.canmove_tick.load(Ordering::Acquire)
```

### 4. Mob flinch without stop packet

```mermaid
sequenceDiagram
    participant P as Player
    participant GL as Game Loop
    participant AT as Atomic Timing
    participant ML as Mob Movement Loop (20ms)
    participant C as Client

    P->>GL: Attack mob
    GL->>GL: Calculate damage
    GL->>AT: canmove_tick = tick + damage_motion
    GL->>C: PacketZcNotifyAct (damage)
    Note over C: Client shows flinch animation

    loop Every 20ms
        ML->>AT: load canmove_tick
        AT-->>ML: tick + 550ms
        alt tick < canmove_tick
            ML->>ML: skip (mob flinching)
        else tick >= canmove_tick
            ML->>ML: resume movement
        end
    end

    Note over GL,ML: No PacketZcStopmove needed!
```

In `map_instance_service.rs` when mob takes damage:
```rust
pub fn mob_being_attacked(&mut self, mob: &mut Mob, tick: u128, damage_motion: u32) {
    // Set atomic timing - movement thread will see this
    mob.timing.canmove_tick.store((tick + damage_motion as u128) as u64, Ordering::Release);

    // Update state
    mob.action = MobAction::Flinching { until: tick + damage_motion as u128 };

    // NO PacketZcStopmove - client shows flinch animation from damage packet
}
```

In mob movement loop:
```rust
if tick < mob.timing.canmove_tick.load(Ordering::Acquire) as u128 {
    continue; // still flinching
}
```

### 5. Add atomic timing structs

```rust
// In character.rs
pub struct Character {
    pub action: CharacterAction,
    pub timing: CharacterTiming,
    // ... rest
}

// In mob.rs
pub struct Mob {
    pub action: MobAction,
    pub timing: MobTiming,
    // ... rest
}
```

## Migration Order

### Phase 1: Fix Timing (no breaking changes)
1. **Unify tick rates to 20ms base** - change character movement from 16ms to 20ms
2. **Add `CharacterTiming` and `MobTiming`** - keep old fields, mirror values to atomics
3. **Fix attack tick init** - one line change in game_loop.rs
4. **Replace sleep with scheduling** - movement handler + movement loop
5. **Fix mob flinch** - use atomic timing instead of stop packet

### Phase 2: State Machine Refactor
6. **Add `CharacterAction` state machine** - replace scattered optionals with enum
7. **Add `MobAction` state machine** - replace scattered state with enum
8. **Remove old optional fields** - cleanup once state machines work

### Step 1 Details: Tick Rate Unification

Files to change:
- `server/src/server/game_loop.rs` - change `MOVEMENT_TICK_RATE` from 16 to 20
- `server/src/server/model/movement.rs` - round delays to 20ms boundaries
- `server/src/server/service/battle_service.rs` - round attack_motion to 20ms

Test: movement should still feel smooth, timing should align across loops.

### Step 2 Details: Add Atomic Timing

Files to change:
- `server/src/server/state/character.rs` - add `CharacterTiming` struct and field
- `server/src/server/state/mob.rs` - add `MobTiming` struct and field

Keep existing fields, just add atomics alongside. Update atomics whenever existing fields change.

### Step 3 Details: Fix Attack Tick Init

Single line in `server/src/server/game_loop.rs:152`:
```rust
character.set_attack(target_id, repeat, tick);  // was: 0
```

### Step 4 Details: Replace Sleep

Files to change:
- `server/src/server/request_handler/movement.rs` - remove sleep, use atomic canmove_tick
- `server/src/server/game_loop.rs` (movement loop) - check start_at before processing

### Step 5 Details: Fix Mob Flinch

Files to change:
- `server/src/server/service/map_instance_service.rs` - set atomic canmove_tick on damage
- `server/src/server/map_instance_loop.rs` - check atomic before mob movement

### Step 6 Details: CharacterAction State Machine

File: `server/src/server/state/character.rs`

Add the enum:
```rust
#[derive(Clone, Debug)]
pub enum CharacterAction {
    Idle,
    Moving { destination: Position, path: Vec<Movement>, started_at: u128 },
    Attacking { target_id: u32, repeat: bool, started_at: u128, last_attack_at: u128, attack_motion: u32 },
    UsingSkill { skill: Box<dyn Skill>, target: Option<u32>, cast_start: u128, cast_end: u128 },
    Sitting,
    Dead,
}
```

Add field to Character:
```rust
pub action: CharacterAction,  // initialize as Idle
```

Add transition methods:
```rust
impl Character {
    pub fn transition_to_idle(&mut self) { ... }
    pub fn transition_to_moving(&mut self, destination: Position, path: Vec<Movement>, tick: u128) { ... }
    pub fn transition_to_attacking(&mut self, target_id: u32, repeat: bool, tick: u128, attack_motion: u32) { ... }
    pub fn transition_to_using_skill(&mut self, skill: Box<dyn Skill>, target: Option<u32>, tick: u128, cast_time: u128) { ... }
    pub fn transition_to_sitting(&mut self) { ... }
    pub fn transition_to_dead(&mut self) { ... }
}
```

Files to update:
- `server/src/server/game_loop.rs` - use transitions instead of set_attack/clear_attack
- `server/src/server/service/server_service.rs` - use transitions for attack/skill
- `server/src/server/service/character/character_service.rs` - use transitions
- `server/src/server/request_handler/action/action.rs` - use transitions

Keep old methods as wrappers initially:
```rust
pub fn set_attack(&mut self, target_id: u32, repeat: bool, tick: u128) {
    self.transition_to_attacking(target_id, repeat, tick, 0);
}

pub fn is_attacking(&self) -> bool {
    matches!(self.action, CharacterAction::Attacking { .. })
}
```

### Step 7 Details: MobAction State Machine

File: `server/src/server/state/mob.rs`

Add the enum:
```rust
#[derive(Clone, Debug)]
pub enum MobAction {
    Idle { idle_until: u128 },
    Moving { destination: Position, path: Vec<Movement>, started_at: u128 },
    Chasing { target_id: u32, last_path_calc: u128 },
    Attacking { target_id: u32, last_attack_at: u128, attack_motion: u32 },
    Flinching { until: u128 },
    Returning { spawn_point: Position },
    Dead { death_time: u128, respawn_at: u128 },
}
```

Add field to Mob:
```rust
pub action: MobAction,  // initialize as Idle { idle_until: 0 }
```

Add transition methods:
```rust
impl Mob {
    pub fn transition_to_idle(&mut self, tick: u128) { ... }
    pub fn transition_to_flinching(&mut self, tick: u128, damage_motion: u32) { ... }
    pub fn transition_to_chasing(&mut self, target_id: u32, tick: u128) { ... }
    pub fn transition_to_attacking(&mut self, target_id: u32, tick: u128) { ... }
    pub fn transition_to_dead(&mut self, tick: u128, respawn_delay: u128) { ... }
}
```

Files to update:
- `server/src/server/map_instance_loop.rs` - use state machine for mob AI
- `server/src/server/service/map_instance_service.rs` - use transitions on damage

### Step 8 Details: Cleanup

Once state machines are working, remove old fields:

From Character:
- `attack: Option<Attack>`
- `skill_in_use: Option<SkillInUse>`
- `sit: bool`

From Mob:
- `last_attacked_at: u128` (now in Flinching state)
- `damage_motion: u32` (now in Flinching state)

Remove wrapper methods, use state machine directly everywhere.

## Tick Rates Refactor

### Current (broken)
- Game loop: 40ms
- Character movement: 16ms ❌ not a multiple of 20
- Mob movement: 20ms

### Target (all multiples of 20ms)
- Game loop: 40ms (2x) ✓
- Character movement: 20ms (1x) ← change this
- Mob movement: 20ms (1x) ✓

### Why 20ms base?
- rathena uses 20ms as `TIMER_MIN_INTERVAL`
- All timing math becomes clean (no fractional ticks)
- Movement speed is 150ms per cell = exactly 7.5 ticks (round to 7 or 8)

### Changes

#### 1. Character movement tick rate

File: `server/src/server/game_loop.rs`

```rust
// OLD
const MOVEMENT_TICK_RATE: u128 = 16;

// NEW
const MOVEMENT_TICK_RATE: u128 = 20;
```

#### 2. Verify mob movement tick rate

File: `server/src/server/map_instance_loop.rs`

```rust
// Should already be 20, verify it's there
const MOVEMENT_TICK_RATE: u128 = 20;
```

#### 3. Update movement delay calculation

File: `server/src/server/model/movement.rs`

Movement delay formula needs to account for 20ms base. Currently uses raw speed value.

```rust
impl Movement {
    // Check this rounds to nearest 20ms tick
    pub fn delay(speed: u16, is_diagonal: bool) -> u128 {
        let base_delay = if is_diagonal {
            (speed as f32 / 0.7) as u128
        } else {
            speed as u128
        };
        // Round to nearest 20ms tick
        ((base_delay + 10) / 20) * 20
    }
}
```

#### 4. Align attack motion to tick boundaries

In battle service when calculating attack motion:

```rust
// Round attack_motion to nearest 20ms
let attack_motion = ((raw_attack_motion + 10) / 20) * 20;
```

This ensures `canmove_tick` and `canact_tick` always land on tick boundaries.

### Tick alignment diagram

```mermaid
gantt
    title Tick Alignment (20ms base)
    dateFormat X
    axisFormat %L

    section Game Loop
    tick 0    :g0, 0, 1
    tick 40   :g1, 40, 1
    tick 80   :g2, 80, 1
    tick 120  :g3, 120, 1

    section Char Movement
    tick 0    :c0, 0, 1
    tick 20   :c1, 20, 1
    tick 40   :c2, 40, 1
    tick 60   :c3, 60, 1
    tick 80   :c4, 80, 1
    tick 100  :c5, 100, 1
    tick 120  :c6, 120, 1

    section Mob Movement
    tick 0    :m0, 0, 1
    tick 20   :m1, 20, 1
    tick 40   :m2, 40, 1
    tick 60   :m3, 60, 1
    tick 80   :m4, 80, 1
    tick 100  :m5, 100, 1
    tick 120  :m6, 120, 1
```

Every 40ms, all three loops align. Movement loops get 2 ticks per game loop tick.

## State Transitions Reference

### Character

```mermaid
stateDiagram-v2
    [*] --> Idle

    Idle --> Moving : move request
    Idle --> Attacking : attack (in range)
    Idle --> UsingSkill : skill request
    Idle --> Sitting : sit request

    Moving --> Idle : arrived
    Moving --> Attacking : attack request
    Moving --> UsingSkill : skill request

    Attacking --> Idle : target gone / cancel
    Attacking --> Moving : chase (out of range)
    Attacking --> UsingSkill : skill request

    UsingSkill --> Idle : skill done / cancelled

    Sitting --> Idle : stand request

    Idle --> Dead : hp <= 0
    Moving --> Dead : hp <= 0
    Attacking --> Dead : hp <= 0
    UsingSkill --> Dead : hp <= 0
    Sitting --> Dead : hp <= 0

    Dead --> Idle : resurrect
```

### Mob

```mermaid
stateDiagram-v2
    [*] --> Idle

    Idle --> Moving : random walk
    Idle --> Chasing : spotted player

    Moving --> Idle : arrived

    Chasing --> Attacking : in range
    Chasing --> Returning : target gone / too far

    Attacking --> Flinching : took damage
    Attacking --> Chasing : target moved
    Attacking --> Returning : target gone

    Flinching --> Idle : flinch done (no target)
    Flinching --> Chasing : flinch done (has target)

    Returning --> Idle : back at spawn

    Idle --> Dead : hp <= 0
    Moving --> Dead : hp <= 0
    Chasing --> Dead : hp <= 0
    Attacking --> Dead : hp <= 0
    Flinching --> Dead : hp <= 0
    Returning --> Dead : hp <= 0

    Dead --> Idle : respawn
```

## Files to Modify

- `server/src/server/state/character.rs` - add CharacterAction, CharacterTiming
- `server/src/server/state/mob.rs` - add MobAction, MobTiming
- `server/src/server/game_loop.rs` - fix attack tick init, use new state
- `server/src/server/request_handler/movement.rs` - remove sleep, use atomic check
- `server/src/server/service/map_instance_service.rs` - mob flinch with atomic timing
- `server/src/server/map_instance_loop.rs` - check atomic canmove_tick for mobs
