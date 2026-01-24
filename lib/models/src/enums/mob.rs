use enum_macro::{WithMaskValueU32, WithNumberValue, WithStringValue};

use crate::enums::{EnumWithMaskValueU32, EnumWithNumberValue, EnumWithStringValue};

#[derive(WithStringValue, WithNumberValue, Debug, Copy, Clone, PartialEq, Eq)]
pub enum MobClass {
    #[value = 0]
    #[value_string = "Class_Normal"]
    Normal,
    #[value_string = "Class_Boss"]
    Boss,
    #[value_string = "Class_Guardian"]
    Guardian,
    #[value_string = "Class_All"]
    All,
}

#[derive(WithStringValue, WithNumberValue, Debug, Copy, Clone, PartialEq, Eq)]
pub enum MobRace {
    #[value_string = "RC_All"]
    #[value_string = "All"]
    #[value = 0]
    All,
    #[value_string = "RC_Angel"]
    #[value_string = "Angel"]
    Angel,
    #[value_string = "RC_Brute"]
    #[value_string = "Brute"]
    Brute,
    #[value_string = "RC_DemiHuman"]
    #[value_string = "DemiHuman"]
    #[value_string = "Demihuman"]
    DemiHuman,
    #[value_string = "RC_Demon"]
    #[value_string = "Demon"]
    Demon,
    #[value_string = "RC_Dragon"]
    #[value_string = "Dragon"]
    Dragon,
    #[value_string = "RC_Fish"]
    #[value_string = "Fish"]
    Fish,
    #[value_string = "RC_Formless"]
    #[value_string = "Formless"]
    Formless,
    #[value_string = "RC_Insect"]
    #[value_string = "Insect"]
    Insect,
    #[value_string = "RC_Plant"]
    #[value_string = "Plant"]
    Plant,
    #[value_string = "RC_Player_Human"]
    PlayerHuman,
    #[value_string = "RC_Player_Doram"]
    PlayerDoram,
    #[value_string = "RC_Undead"]
    #[value_string = "Undead"]
    RUndead,
}

#[derive(WithStringValue, WithNumberValue, Debug, Copy, Clone, PartialEq, Eq)]
pub enum MobGroup {
    #[value_string = "RC2_Goblin"]
    #[value = 0]
    Goblin,
    #[value_string = "RC2_Kobold"]
    Kobold,
    #[value_string = "RC2_Golem"]
    Golem,
    #[value_string = "RC2_Orc"]
    Orc,
    #[value_string = "RC2_Guardian"]
    Guardian,
    #[value_string = "RC2_Ninja"]
    Ninja,
    #[value_string = "RC2_GVG"]
    GVG,
    #[value_string = "RC2_Battlefield"]
    Battlefield,
    #[value_string = "RC2_Treasure"]
    Treasure,
    #[value_string = "RC2_BioLab"]
    BioLab,
    #[value_string = "RC2_Manuk"]
    Manuk,
    #[value_string = "RC2_Splendide"]
    Splendide,
    #[value_string = "RC2_Scaraba"]
    Scaraba,
    #[value_string = "RC2_Clocktower"]
    Clocktower,
    #[value_string = "RC2_Thanatos"]
    Thanatos,
    #[value_string = "RC2_Faceworm"]
    Faceworm,
    #[value_string = "RC2_Hearthunter"]
    Hearthunter,
    #[value_string = "RC2_Rockridge"]
    Rockridge,
    #[value_string = "RC2_Werner_Lab"]
    WernerLab,
    #[value_string = "RC2_Temple_Demon"]
    TempleDemon,
    #[value_string = "RC2_Illusion_Vampire"]
    IllusionVampire,
    #[value_string = "RC2_Malangdo"]
    Malangdo,
    #[value_string = "RC2_Rachel_Sanctuary"]
    RachelSanctuary,
}

/// Monster behavior mode flags
/// See docs/monster-ai.md for detailed documentation
#[derive(WithMaskValueU32, Debug, Copy, Clone, PartialEq, Eq)]
pub enum MobMode {
    #[mask_value = 1]
    CanMove,
    Looter,
    Aggressive,
    Assist,
    CastSensorIdle,
    NoRandomWalk,
    NoCast,
    CanAttack,
    #[mask_value = 512]
    CastSensorChase,
    ChangeChase,
    Angry,
    ChangeTargetMelee,
    ChangeTargetChase,
    TargetWeak,
    RandomTarget,
    #[mask_value = 65536]
    IgnoreMelee,
    IgnoreMagic,
    IgnoreRanged,
    Mvp,
    IgnoreMisc,
    KnockBackImmune,
    TeleportBlock,
    #[mask_value = 16777216]
    FixedItemDrop,
    Detector,
    StatusImmune,
    SkillImmune,
}

impl MobMode {
    pub fn is_aggressive(mode: u32) -> bool {
        mode & MobMode::Aggressive.as_flag() != 0
    }

    pub fn is_assist(mode: u32) -> bool {
        mode & MobMode::Assist.as_flag() != 0
    }

    pub fn can_move(mode: u32) -> bool {
        mode & MobMode::CanMove.as_flag() != 0
    }

    pub fn can_attack(mode: u32) -> bool {
        mode & MobMode::CanAttack.as_flag() != 0
    }

    pub fn is_looter(mode: u32) -> bool {
        mode & MobMode::Looter.as_flag() != 0
    }

    pub fn has_cast_sensor_idle(mode: u32) -> bool {
        mode & MobMode::CastSensorIdle.as_flag() != 0
    }

    pub fn has_cast_sensor_chase(mode: u32) -> bool {
        mode & MobMode::CastSensorChase.as_flag() != 0
    }

    pub fn can_change_target_melee(mode: u32) -> bool {
        mode & MobMode::ChangeTargetMelee.as_flag() != 0
    }

    pub fn can_change_target_chase(mode: u32) -> bool {
        mode & MobMode::ChangeTargetChase.as_flag() != 0
    }

    pub fn is_angry(mode: u32) -> bool {
        mode & MobMode::Angry.as_flag() != 0
    }

    pub fn targets_weak(mode: u32) -> bool {
        mode & MobMode::TargetWeak.as_flag() != 0
    }

    pub fn has_random_target(mode: u32) -> bool {
        mode & MobMode::RandomTarget.as_flag() != 0
    }

    pub fn is_detector(mode: u32) -> bool {
        mode & MobMode::Detector.as_flag() != 0
    }

    pub fn is_mvp(mode: u32) -> bool {
        mode & MobMode::Mvp.as_flag() != 0
    }
}
