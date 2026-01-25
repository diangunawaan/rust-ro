use models::enums::element::Element;
use models::enums::mob::MobRace;
use models::enums::size::Size;
use models::status::StatusSnapshot;

use crate::server::model::status::StatusFromDb;
use crate::server::service::global_config_service::GlobalConfigService;
use crate::server::state::mob::Mob;

/// Creates a simple StatusSnapshot for test mobs with basic stats
pub fn create_test_mob_status(hp: u32, atk1: u16, atk2: u16) -> StatusSnapshot {
    StatusSnapshot::new_for_mob(
        1002, // mob_id (PORING)
        hp,
        0,    // sp
        hp,   // max_hp
        0,    // max_sp
        1,    // str
        1,    // agi
        1,    // vit
        1,    // int
        1,    // dex
        1,    // luk
        atk1,
        atk2,
        atk1, // matk1
        atk2, // matk2
        400,  // speed
        0,    // def
        0,    // mdef
        Size::Small,
        Element::Water,
        MobRace::Plant,
        1, // element_level
    )
}

pub fn create_mob(map_item_id: u32, mob_name: &str) -> Mob {
    let mob = GlobalConfigService::instance().get_mob_by_name(mob_name);
    Mob::new(
        map_item_id,
        90,
        90,
        mob.id as i16,
        0,
        mob.name.clone(),
        mob.name_english.clone(),
        mob.damage_motion as u32,
        StatusFromDb::from_mob_model(mob),
        mob.mode as u32,
        mob.range1 as u16,
        mob.range3 as u16,
        mob.atk_delay as u32,
        mob.atk_motion as u32,
        mob.atk1 as u16,
        mob.atk2 as u16,
    )
}
pub fn create_mob_by_id(map_item_id: u32, mob_id: u32) -> Mob {
    let mob = GlobalConfigService::instance().get_mob(mob_id as i32);
    Mob::new(
        map_item_id,
        90,
        90,
        mob.id as i16,
        0,
        mob.name.clone(),
        mob.name_english.clone(),
        mob.damage_motion as u32,
        StatusFromDb::from_mob_model(mob),
        mob.mode as u32,
        mob.range1 as u16,
        mob.range3 as u16,
        mob.atk_delay as u32,
        mob.atk_motion as u32,
        mob.atk1 as u16,
        mob.atk2 as u16,
    )
}

pub fn create_mob_with_mode(map_item_id: u32, mob_name: &str, mode: u32) -> Mob {
    let mob = GlobalConfigService::instance().get_mob_by_name(mob_name);
    Mob::new(
        map_item_id,
        90,
        90,
        mob.id as i16,
        0,
        mob.name.clone(),
        mob.name_english.clone(),
        mob.damage_motion as u32,
        StatusFromDb::from_mob_model(mob),
        mode,
        mob.range1 as u16,
        mob.range3 as u16,
        mob.atk_delay as u32,
        mob.atk_motion as u32,
        mob.atk1 as u16,
        mob.atk2 as u16,
    )
}

pub fn create_mob_at_position(map_item_id: u32, mob_name: &str, x: u16, y: u16) -> Mob {
    let mob = GlobalConfigService::instance().get_mob_by_name(mob_name);
    Mob::new(
        map_item_id,
        x,
        y,
        mob.id as i16,
        0,
        mob.name.clone(),
        mob.name_english.clone(),
        mob.damage_motion as u32,
        StatusFromDb::from_mob_model(mob),
        mob.mode as u32,
        mob.range1 as u16,
        mob.range3 as u16,
        mob.atk_delay as u32,
        mob.atk_motion as u32,
        mob.atk1 as u16,
        mob.atk2 as u16,
    )
}
