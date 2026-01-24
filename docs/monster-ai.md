# Monster AI System

This document describes the Ragnarok Online monster AI system implementation based on official game mechanics research.

## Monster Modes (Bitflags)

Monster behavior is controlled by mode bitflags. Each flag enables specific AI behaviors:

| Flag | Hex Value | Description |
|------|-----------|-------------|
| `CANMOVE` | 0x0000001 | Enables creature movement and chasing |
| `LOOTER` | 0x0000002 | Loots items from the ground when idle |
| `AGGRESSIVE` | 0x0000004 | Seeks nearby players to attack |
| `ASSIST` | 0x0000008 | Assists same-type monsters by attacking their targets |
| `CASTSENSOR_IDLE` | 0x0000010 | Detects players casting spells during idle |
| `NORANDOMWALK` | 0x0000020 | Disables random wandering during idle |
| `NOCAST` | 0x0000040 | Unable to cast skills (normal attacks only) |
| `CANATTACK` | 0x0000080 | Enables attack capability |
| `CASTSENSOR_CHASE` | 0x0000200 | Detects and targets spellcasters while chasing |
| `CHANGECHASE` | 0x0000400 | Changes chase behavior during pursuit |
| `ANGRY` | 0x0000800 | Hyper-active state with distinct behavior before/after being hit |
| `CHANGETARGET_MELEE` | 0x0001000 | Changes target while in melee range |
| `CHANGETARGET_CHASE` | 0x0002000 | Changes target while chasing |
| `TARGETWEAK` | 0x0004000 | Prioritizes weakest enemies |
| `RANDOMTARGET` | 0x0008000 | Picks random targets in range for each attack |
| `IGNOREMELEE` | 0x0010000 | Takes only 1 HP damage from physical attacks |
| `IGNOREMAGIC` | 0x0020000 | Takes only 1 HP damage from magic attacks |
| `IGNORERANGED` | 0x0040000 | Takes only 1 HP damage from ranged attacks |
| `MVP` | 0x0080000 | Boss flag; resistant to Coma status |
| `IGNOREMISC` | 0x0100000 | Takes only 1 HP damage from miscellaneous attacks |
| `KNOCKBACKIMMUNE` | 0x0200000 | Cannot be knocked back |
| `TELEPORTBLOCK` | 0x0400000 | Cannot be teleported |
| `FIXEDITEMDROP` | 0x1000000 | Drops not affected by item drop modifiers |
| `DETECTOR` | 0x2000000 | Can detect and attack hidden/cloaked players |
| `STATUSIMMUNE` | 0x4000000 | Immune to status effects |
| `SKILLIMMUNE` | 0x8000000 | Immune to skill effects |

## AI Type Presets

Common AI type combinations used in the database:

| Type | Behavior | Mode Value |
|------|----------|------------|
| 01 | Passive | 0x0081 |
| 02 | Passive, Looter | 0x0083 |
| 03 | Passive, Assist, Change-target Melee | 0x1089 |
| 04 | Angry, Change-target Melee/Chase | 0x3885 |
| 05 | Aggressive, Change-target Chase | 0x2085 |
| 06 | Passive, Immobile, Can't Attack (Plants) | 0x0000 |
| 07 | Passive, Looter, Assist, Change-target Melee | 0x108B |
| 08 | Aggressive, Change-target Melee/Chase, Target Weak | 0x7085 |
| 09 | Aggressive, Change-target Melee/Chase, Cast Sensor Idle | 0x3095 |
| 10 | Aggressive, Immobile | 0x0084 |
| 13 | Aggressive, Change-target Melee/Chase, Assist | 0x308D |
| 17 | Passive, Cast Sensor Idle | 0x0091 |
| 19 | Aggressive, Change-target Melee/Chase, Cast Sensor Idle | 0x3095 |
| 20 | Aggressive, Change-target Melee/Chase, Cast Sensor Idle/Chase | 0x3295 |
| 21 | Aggressive, Full options | 0x3695 |
| 24 | Passive, No Random Walk (Slave) | 0x00A1 |
| 25 | Passive, Can't Attack (Pet) | 0x0001 |
| 26 | Aggressive, Full options, Random Target | 0xB695 |
| 27 | Aggressive, Immobile, Random Target | 0x8084 |

## AI State Machine

The monster AI uses a finite state machine:

```
                    ┌──────────────┐
                    │    IDLE      │
                    │  (default)   │
                    └──────┬───────┘
                           │
         ┌─────────────────┼─────────────────┐
         │                 │                 │
         ▼                 ▼                 ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   RMOVE      │  │   SEARCH     │  │  MOVEITEM    │
│(random walk) │  │(find target) │  │  (looting)   │
└──────────────┘  └──────┬───────┘  └──────────────┘
                         │
                         ▼
                  ┌──────────────┐
                  │    RUSH      │
                  │  (chasing)   │
                  └──────┬───────┘
                         │
                         ▼
                  ┌──────────────┐
                  │   BERSERK    │
                  │ (attacking)  │
                  └──────────────┘
```

### State Descriptions

| State | Description |
|-------|-------------|
| **IDLE** | Default state with no active action |
| **RMOVE** | Random walking during non-combat periods |
| **RUSH** | Actively pursuing a target player |
| **BERSERK** | In melee range, attacking |
| **ANGRY** | Hyper-active state with different behavior before/after being hit |
| **SEARCH** | Actively scanning for targets |
| **FOLLOW_SEARCH** | Scanning while following an ally |
| **MOVEITEM** | Moving toward and looting items on the ground |
| **FOLLOW** | Traveling alongside a parent unit |
| **ABNORMAL** | Incapacitated by control-loss effects |
| **DEAD** | Removed from map during respawn timer |

## Detection & Targeting

### Range-Based Detection

- **Aggressive Range**: Initial detection radius (varies by monster, typically ~10 cells)
- **Chase Range**: Maximum pursuit distance (typically ~12 cells, defined in `range3`)
- **Attack Range**: Melee/ranged attack distance (defined in `range1`)

### Target Selection Priority

1. **Passive mobs**: Don't initiate combat, only attack when attacked first
2. **Aggressive mobs**: Seek nearest player within detection range
3. **Cast Sensor mobs**: Immediately target visible spellcasters
4. **Assist mobs**: Join allies attacking the same target
5. **Weak-targeting mobs**: Prioritize low-health enemies
6. **Random-target mobs**: Pick new random target for each attack

### Special Detection

- **Detector mobs**: Can attack hidden/cloaked players (Hiding, Cloaking, Stalk)
- **Cast Sensor**: Triggers on visible spellcasting
- Monsters recalculate targets every 32 cells moved or when near path end

## Attack Mechanics

### Timing Parameters

| Field | Description |
|-------|-------------|
| `atk_delay` | Minimum time between attacks (ASPD) |
| `atk_motion` | Attack animation duration |
| `damage_motion` | Flinch animation duration when hit |

### Damage Calculation

Basic mob attack damage:
```
damage = random(atk1, atk2) * modifiers - target_defense
```

Where modifiers include:
- Element advantage/disadvantage
- Size modifiers
- Race modifiers

## Behavior Patterns

### Aggressive Behavior
1. Detect player within `chase_range`
2. If in `attack_range`: Attack immediately
3. If out of `attack_range`: Chase until in range
4. If target escapes `chase_range`: Return to idle

### Passive Behavior
1. Remain idle until attacked
2. Store attacker as target
3. Chase and attack the attacker
4. May return to idle after losing target

### Assist Behavior
1. When same-type mob is attacked nearby
2. All assist mobs within range target the attacker
3. Chase and attack together

### Angry Behavior
1. Has two distinct skill sets (before/after being attacked)
2. Switches behavior when first hit
3. May become more aggressive after being attacked

## Configuration Settings

| Setting | Description |
|---------|-------------|
| `mob_skill_rate` | Probability of skill usage (default: 100%) |
| `mob_skill_delay` | Adjusts skill recast timing (default: 100%) |
| `monster_chase_refresh` | Path recalculation frequency (default: 32 cells) |
| `mob_move_frequency_when_player_around` | Random movement probability when players nearby |
| `mob_move_frequency_when_no_player_around` | Random movement probability when no players |

## References

- [Ragnarok Research Lab - Creature AI](https://ragnarokresearchlab.github.io/game-mechanics/creature-ai/)
- [rAthena Monster Modes Documentation](https://github.com/rathena/rathena/blob/master/doc/mob_db_mode_list.txt)
- [rAthena Monster Configuration](https://github.com/rathena/rathena/blob/master/conf/battle/monster.conf)
- [iRO Wiki - Monster](https://irowiki.org/wiki/Monster)
- [iRO Wiki - AI](https://irowiki.org/wiki/AI)
