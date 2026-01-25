#![allow(dead_code)]

use crate::server::model::events::client_notification::Notification;
use crate::server::model::events::persistence_event::PersistenceEvent;
use crate::server::service::global_config_service::GlobalConfigService;
use crate::server::service::mob_service::MobService;
use crate::tests::common;
use crate::tests::common::sync_helper::CountDownLatch;
use crate::tests::common::{create_mpsc, TestContext};

struct MobServiceTestContext {
    test_context: TestContext,
    mob_service: MobService,
}

fn before_each() -> MobServiceTestContext {
    common::before_all();
    let (client_notification_sender, client_notification_receiver) = create_mpsc::<Notification>();
    let (persistence_event_sender, persistence_event_receiver) = create_mpsc::<PersistenceEvent>();
    let count_down_latch = CountDownLatch::new(0);
    MobServiceTestContext {
        test_context: TestContext::new(
            client_notification_sender.clone(),
            client_notification_receiver,
            persistence_event_sender.clone(),
            persistence_event_receiver,
            count_down_latch,
        ),
        mob_service: MobService::new(client_notification_sender, GlobalConfigService::instance()),
    }
}

#[cfg(test)]
#[cfg(not(feature = "integration_tests"))]
mod tests {
    use models::enums::mob::MobMode;
    use models::enums::EnumWithMaskValueU32;
    use models::position::Position;

    use crate::server::model::map_item::{MapItem, MapItemSnapshot, MapItemType};
    use crate::server::service::mob_service::MobAIAction;
    use crate::server::state::mob::{Mob, MobAction};
    use crate::tests::common::mob_helper::create_test_mob_status;
    use crate::tests::mob_service_test::before_each;

    fn create_mob_with_mode_at(id: u32, x: u16, y: u16, mode: u32, attack_range: u16, chase_range: u16) -> Mob {
        Mob::new(
            id,
            x,
            y,
            1002,
            0,
            "PORING".to_string(),
            "Poring".to_string(),
            480,
            create_test_mob_status(50, 7, 10),
            mode,
            attack_range,
            chase_range,
            1872, // atk_delay
            672,  // atk_motion
            7,
            10,
        )
    }

    fn create_character_snapshot(id: u32, x: u16, y: u16) -> MapItemSnapshot {
        MapItemSnapshot {
            map_item: MapItem::new(id, 0, MapItemType::Character),
            position: Position { x, y, dir: 0 },
        }
    }

    fn aggressive_mode() -> u32 {
        MobMode::CanMove.as_flag() | MobMode::CanAttack.as_flag() | MobMode::Aggressive.as_flag()
    }

    fn passive_mode() -> u32 {
        MobMode::CanMove.as_flag() | MobMode::CanAttack.as_flag()
    }

    #[test]
    fn test_aggressive_mob_attacks_player_in_attack_range() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, aggressive_mode(), 1, 12);
        let characters = vec![create_character_snapshot(100, 51, 50)]; // 1 cell away, in attack range
        let cells: Vec<u16> = vec![1; 100 * 100];

        let action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 10000);

        match action {
            Some(MobAIAction::Attack(attack)) => {
                assert_eq!(attack.mob_id, 1);
                assert_eq!(attack.target_char_id, 100);
                assert!(attack.damage >= 7 && attack.damage <= 10);
            }
            _ => panic!("Expected Attack action"),
        }
        match mob.action {
            MobAction::Attacking { target_id, .. } => assert_eq!(target_id, 100),
            _ => panic!("Expected Attacking state"),
        }
    }

    #[test]
    fn test_aggressive_mob_chases_player_in_chase_range_but_outside_attack_range() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, aggressive_mode(), 1, 12);
        let characters = vec![create_character_snapshot(100, 55, 50)]; // 5 cells away
        let cells: Vec<u16> = vec![1; 100 * 100];

        let action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 10000);

        assert!(matches!(action, Some(MobAIAction::Move(_))));
        match mob.action {
            MobAction::Chasing { target_id } => assert_eq!(target_id, 100),
            _ => panic!("Expected Chasing state with target_id 100"),
        }
    }

    #[test]
    fn test_aggressive_mob_ignores_player_outside_chase_range() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, aggressive_mode(), 1, 12);
        let characters = vec![create_character_snapshot(100, 70, 50)]; // 20 cells away
        let cells: Vec<u16> = vec![1; 100 * 100];

        let _action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 10000);

        assert!(matches!(mob.action, MobAction::Idle | MobAction::Moving));
        assert!(mob.target_id.is_none());
    }

    #[test]
    fn test_passive_mob_does_not_attack_player_unprovoked() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, passive_mode(), 1, 12);
        let characters = vec![create_character_snapshot(100, 51, 50)]; // In attack range
        let cells: Vec<u16> = vec![1; 100 * 100];

        let action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 10000);

        assert!(!matches!(action, Some(MobAIAction::Attack(_))));
        assert!(!matches!(mob.action, MobAction::Attacking { .. } | MobAction::Chasing { .. }));
        assert!(mob.target_id.is_none());
    }

    #[test]
    fn test_passive_mob_retaliates_after_being_attacked() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, passive_mode(), 1, 12);
        mob.target_id = Some(100); // Simulates being attacked
        let characters = vec![create_character_snapshot(100, 51, 50)];
        let cells: Vec<u16> = vec![1; 100 * 100];

        let action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 10000);

        match action {
            Some(MobAIAction::Attack(attack)) => {
                assert_eq!(attack.target_char_id, 100);
            }
            _ => panic!("Expected passive mob to retaliate"),
        }
        assert!(matches!(mob.action, MobAction::Attacking { target_id: 100, .. }));
    }

    #[test]
    fn test_mob_loses_target_when_player_leaves_map() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, aggressive_mode(), 1, 12);
        mob.action = MobAction::Chasing { target_id: 100 };
        mob.target_id = Some(100);
        let characters: Vec<MapItemSnapshot> = vec![]; // Player left
        let cells: Vec<u16> = vec![1; 100 * 100];

        let action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 10000);

        assert!(action.is_none() || matches!(action, Some(MobAIAction::Move(_))));
        assert!(matches!(mob.action, MobAction::Idle));
        assert!(mob.target_id.is_none());
    }

    #[test]
    fn test_chasing_mob_loses_target_when_player_escapes_chase_range() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, aggressive_mode(), 1, 12);
        mob.action = MobAction::Chasing { target_id: 100 };
        let characters = vec![create_character_snapshot(100, 70, 50)]; // 20 cells away, outside chase_range
        let cells: Vec<u16> = vec![1; 100 * 100];

        let _action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 10000);

        assert!(matches!(mob.action, MobAction::Idle));
        assert!(mob.target_id.is_none());
    }

    #[test]
    fn test_chasing_mob_transitions_to_attacking_when_reaching_target() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, aggressive_mode(), 1, 12);
        mob.action = MobAction::Chasing { target_id: 100 };
        let characters = vec![create_character_snapshot(100, 51, 50)]; // Now in attack range
        let cells: Vec<u16> = vec![1; 100 * 100];

        let action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 10000);

        assert!(matches!(action, Some(MobAIAction::Attack(_))));
        match mob.action {
            MobAction::Attacking { target_id, .. } => assert_eq!(target_id, 100),
            _ => panic!("Expected transition to Attacking"),
        }
    }

    #[test]
    fn test_attacking_mob_respects_attack_delay() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, aggressive_mode(), 1, 12);
        mob.action = MobAction::Attacking {
            target_id: 100,
            last_attack_at: 10000,
        };
        let characters = vec![create_character_snapshot(100, 51, 50)];
        let cells: Vec<u16> = vec![1; 100 * 100];

        // Try attack 500ms after last attack (atk_delay is 1872ms)
        let action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 10500);

        assert!(action.is_none());
        // State should remain Attacking
        assert!(matches!(mob.action, MobAction::Attacking { target_id: 100, .. }));
    }

    #[test]
    fn test_attacking_mob_attacks_again_after_delay() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, aggressive_mode(), 1, 12);
        mob.action = MobAction::Attacking {
            target_id: 100,
            last_attack_at: 10000,
        };
        let characters = vec![create_character_snapshot(100, 51, 50)];
        let cells: Vec<u16> = vec![1; 100 * 100];

        // Attack 2000ms after last attack (atk_delay is 1872ms)
        let action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 12000);

        match action {
            Some(MobAIAction::Attack(attack)) => {
                assert_eq!(attack.target_char_id, 100);
            }
            _ => panic!("Expected attack after delay"),
        }
    }

    #[test]
    fn test_attacking_mob_switches_to_chasing_when_target_moves_away() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, aggressive_mode(), 1, 12);
        mob.action = MobAction::Attacking {
            target_id: 100,
            last_attack_at: 10000,
        };
        let characters = vec![create_character_snapshot(100, 55, 50)]; // Moved out of attack range
        let cells: Vec<u16> = vec![1; 100 * 100];

        let _action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 12000);

        match mob.action {
            MobAction::Chasing { target_id } => assert_eq!(target_id, 100),
            _ => panic!("Expected transition to Chasing when target moved away"),
        }
    }

    #[test]
    fn test_flinching_mob_cannot_act() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, aggressive_mode(), 1, 12);
        mob.action = MobAction::Flinching { until: 20000 };
        mob.target_id = Some(100);
        let characters = vec![create_character_snapshot(100, 51, 50)];
        let cells: Vec<u16> = vec![1; 100 * 100];

        let action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 10000);

        assert!(action.is_none());
        assert!(matches!(mob.action, MobAction::Flinching { .. }));
        // Target should be preserved for after flinch
        assert_eq!(mob.target_id, Some(100));
    }

    #[test]
    fn test_flinching_mob_recovers_and_retaliates() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, passive_mode(), 1, 12);
        mob.action = MobAction::Flinching { until: 10000 };
        mob.target_id = Some(100);
        let characters = vec![create_character_snapshot(100, 51, 50)];
        let cells: Vec<u16> = vec![1; 100 * 100];

        // Call after flinch ends
        let action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 11000);

        assert!(matches!(action, Some(MobAIAction::Attack(_))));
        assert!(matches!(mob.action, MobAction::Attacking { target_id: 100, .. }));
    }

    #[test]
    fn test_passive_mob_clears_target_when_attacker_leaves() {
        let context = before_each();
        let mut mob = create_mob_with_mode_at(1, 50, 50, passive_mode(), 1, 12);
        mob.target_id = Some(100);
        let characters: Vec<MapItemSnapshot> = vec![]; // Attacker left
        let cells: Vec<u16> = vec![1; 100 * 100];

        let _action = context.mob_service.action_ai(&mut mob, &characters, &cells, 100, 100, 10000);

        assert!(mob.target_id.is_none());
    }
}
