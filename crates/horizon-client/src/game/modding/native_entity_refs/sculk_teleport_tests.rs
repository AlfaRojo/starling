use horizon_client::mobs::{Damage, Mob};
use horizon_core::geometry::Vec3;
use std::sync::{Arc, Mutex};

/// A hook that teleports a later, stationary mob during the scan (after its
/// position was sampled) must not count as that mob walking back to the old
/// spot, on this scan or the next (`Entity.teleport` walks no distance).
#[test]
#[ignore = "daily"]
fn a_hook_teleporting_a_later_stationary_mob_posts_no_step() {
    type Shared = Arc<Mutex<(bool, Vec<String>)>>;
    struct Teleporter(Shared);
    #[mod_api::__subscriptions]
    impl mod_api::Hooks for Teleporter {
        fn vanilla_game_event(
            &self,
            host: &mut dyn mod_api::ServerHost,
            event: &mut mod_api::VanillaGameEventEvent<'_>,
        ) -> mod_api::Flow {
            let mut shared = self.0.lock().unwrap();
            shared.1.push(event.event.to_owned());
            if event.event == "minecraft:entity_damage" && std::mem::take(&mut shared.0) {
                let tp = "tp @e[type=minecraft:pig,sort=furthest,limit=1] 10.5 100 0.5";
                assert!(host.commands().run_command(tp));
            }
            mod_api::Flow::Continue
        }
    }
    let shared: Shared = Arc::default();
    let mods = vec![mod_api::ModEntry::new(
        "teleporter",
        Box::new(Teleporter(shared.clone())),
    )]
    .into();
    mod_platform::testing::with_mods(mods, || {
        crate::game::native_guest_turn_tests::with_guests(|game| {
            let pig = game.res.mob_types().get("minecraft:pig").cloned().unwrap();
            let at = |x| Vec3::new(x, 100.0, 0.5);
            game.mobs.push(Mob::new(1, pig.clone(), at(0.5), 0.0));
            game.mobs.push(Mob::new(2, pig, at(3.5), 0.0));
            for m in game.mobs.iter_mut() {
                m.on_ground = true;
            }
            game.reset_entity_residency();
            game.world.chunks.insert(
                (0, 0),
                std::sync::Arc::new(horizon_client::chunk::Chunk::empty(0, 0)),
            );
            game.block_ticks
                .add_empty_chunk(horizon_core::position::ChunkPos::new(0, 0))
                .unwrap();
            let _ = game.run_command("forceload add 0 0");
            // A block under the old spot, so a walked step would be heard.
            let stone = game
                .pack
                .assets
                .registry
                .default_state("minecraft:stone")
                .unwrap();
            game.world.set(3, 99, 0, stone).unwrap();
            assert!(!game.res.facts().is_air(game.world.block(3, 99, 0)));
            game.sculk_prepare();
            game.tick_mob_sculk_events();
            shared.lock().unwrap().1.clear();

            // Mob 1 is hurt this tick; its damage event teleports mob 2.
            game.mobs[0].record_damage(Damage::fall(), 5.0);
            shared.lock().unwrap().0 = true;
            game.tick_mob_sculk_events();
            assert_eq!(game.mobs[1].position, at(10.5), "the hook moved it");
            game.tick_mob_sculk_events();
            let steps = shared
                .lock()
                .unwrap()
                .1
                .iter()
                .filter(|e| *e == "minecraft:step")
                .count();
            assert_eq!(steps, 0, "no step on either scan");
        });
    });
}
