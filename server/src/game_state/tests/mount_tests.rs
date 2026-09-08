use super::*;

async fn rider(game: &GameState) -> PlayerId {
    let player = make_player("Rider", 0.0, 0.0);
    let id = player.id;
    game.add_player(player).await;
    game.inventories.write().await.insert(
        id,
        PlayerInventory {
            bag: vec![bag_item(1, "horse_reins", 1)],
            ..Default::default()
        },
    );
    id
}

#[tokio::test]
async fn horse_reins_toggle_without_consumption_and_broadcast() {
    let game = make_test_game_state("horse_toggle");
    let id = rider(&game).await;
    let mut rx = game.register_direct_channel(&id).await;
    game.use_item(&id, 1).await;
    assert!(game.players.read().await[&id].mounted);
    assert!(drain(&mut rx)
        .iter()
        .any(|m| matches!(m, ServerMessage::PlayerMountChanged { mounted: true, .. })));
    game.use_item(&id, 1).await;
    assert!(!game.players.read().await[&id].mounted);
    assert_eq!(
        game.get_player_inventory(&id).await.unwrap().bag[0].quantity,
        1
    );
}

#[tokio::test]
async fn horse_movement_is_twice_as_fast_and_losing_reins_dismounts() {
    let game = make_test_game_state("horse_speed");
    let id = rider(&game).await;
    game.players.write().await.get_mut(&id).unwrap().rotation = std::f32::consts::FRAC_PI_2;
    game.use_item(&id, 1).await;
    game.update_player_position(
        &id,
        move_cmd(
            Position {
                x: 20.0,
                y: 5.0,
                z: 0.0,
            },
            false,
        ),
        false,
    )
    .await;
    game.tick_player_movement(1.0).await;
    assert!((game.players.read().await[&id].position.x - 6.0).abs() < 0.01);
    game.inventories
        .write()
        .await
        .get_mut(&id)
        .unwrap()
        .bag
        .clear();
    game.tick_player_movement(1.0).await;
    let player = game.players.read().await[&id].clone();
    assert!(!player.mounted);
    assert!((player.position.x - 9.0).abs() < 0.01);
}

#[tokio::test]
async fn horse_turns_along_an_arc_and_accepts_a_new_direction() {
    let game = make_test_game_state("horse_turn");
    let id = rider(&game).await;
    game.use_item(&id, 1).await;
    game.update_player_position(&id, move_cmd(pos(20.0), false), false)
        .await;
    game.tick_player_movement(0.2).await;
    let p = game.players.read().await[&id].clone();
    assert!((p.position.x - 0.65 * (1.0 - (std::f32::consts::PI / 6.0).cos())).abs() < 1e-4);
    assert!((p.position.z - 0.325).abs() < 1e-4);
    assert!((p.rotation - std::f32::consts::PI / 6.0).abs() < 1e-5);
    game.turn_horse(&id, -std::f32::consts::FRAC_PI_2, false)
        .await;
    game.tick_player_movement(0.1).await;
    let turned = game.players.read().await[&id].clone();
    assert!(turned.rotation < p.rotation);
    assert!(turned.position.dist_xz_sq(&p.position) > 0.0);
    game.tick_player_movement(1.0).await;
    assert!((game.players.read().await[&id].rotation + std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    assert!(!game.movement_intents.read().await.contains_key(&id));
    game.turn_horse(&id, f32::NAN, false).await;
    assert!(!game.movement_intents.read().await.contains_key(&id));
    game.use_item(&id, 1).await;
    game.turn_horse(&id, 0.0, false).await;
    assert!(!game.movement_intents.read().await.contains_key(&id));
}

#[tokio::test]
async fn horse_travel_matches_small_ticks_and_cannot_skip_a_waypoint_turn() {
    let game = make_test_game_state("horse_turn_ticks");
    let id = rider(&game).await;
    game.use_item(&id, 1).await;
    game.update_player_position(&id, move_cmd(pos(20.0), false), false)
        .await;
    for _ in 0..10 {
        game.tick_player_movement(0.1).await;
    }
    let p = game.players.read().await[&id].clone();
    let coarse = make_test_game_state("horse_turn_coarse");
    let coarse_id = rider(&coarse).await;
    coarse.use_item(&coarse_id, 1).await;
    coarse
        .update_player_position(&coarse_id, move_cmd(pos(20.0), false), false)
        .await;
    coarse.tick_player_movement(1.0).await;
    let other = coarse.players.read().await[&coarse_id].clone();
    assert!(p.position.dist_xz_sq(&other.position) < 1e-5);
    assert!((p.rotation - other.rotation).abs() < 1e-4);
    game.update_player_position(&id, move_cmd(p.position, false), false)
        .await;
    game.update_player_position(
        &id,
        move_cmd(
            Position {
                z: -20.0,
                ..p.position
            },
            true,
        ),
        false,
    )
    .await;
    game.tick_player_movement(0.2).await;
    let after = game.players.read().await[&id].clone();
    assert!(after.position.dist_xz_sq(&p.position) > 0.01);
    assert!((after.rotation - std::f32::consts::PI).abs() > 0.5);
    game.stop_horse(&id).await;
    game.tick_player_movement(1.0).await;
    assert_eq!(game.players.read().await[&id].position, after.position);
    assert_eq!(game.players.read().await[&id].rotation, after.rotation);
}

#[tokio::test]
async fn horse_turning_does_not_bypass_solid_furniture() {
    let game = make_test_game_state("horse_turn_collision");
    let id = rider(&game).await;
    let start = Position {
        x: 0.5,
        y: 5.0,
        z: 4.5,
    };
    game.players.write().await.get_mut(&id).unwrap().position = start;
    game.players.write().await.get_mut(&id).unwrap().rotation = 0.0;
    game.use_item(&id, 1).await;
    assert!(game.players.read().await[&id].mounted);
    game.sync_region_furniture(0, 0, &[table_placement(0.5, 5.5)]);
    game.turn_horse(&id, -std::f32::consts::FRAC_PI_2, false)
        .await;
    game.tick_player_movement(2.0).await;
    let p = game.players.read().await[&id].clone();
    assert!(
        p.position.z < 5.0,
        "arc crossed the table boundary: {:?}",
        p.position
    );
    assert!(p.position.z > start.z);
    assert!(p.rotation > -std::f32::consts::FRAC_PI_2);
    assert!(!game.movement_intents.read().await.contains_key(&id));
}

#[tokio::test]
async fn horse_mount_rejects_defeat_dungeons_combat_and_water() {
    let game = make_test_game_state("horse_guards");
    let id = rider(&game).await;
    for state in 0..4 {
        {
            let mut players = game.players.write().await;
            let p = players.get_mut(&id).unwrap();
            p.health = if state == 0 { 0 } else { 10 };
            p.floor_level = if state == 1 { -1 } else { 0 };
            p.last_combat_at = if state == 2 { GameState::now_ms() } else { 0 };
            p.position.x = if state == 3 { -50.0 } else { 0.0 };
        }
        game.use_item(&id, 1).await;
        assert!(!game.players.read().await[&id].mounted);
    }
    assert_eq!(
        game.get_player_inventory(&id).await.unwrap().bag[0].quantity,
        1
    );
}

#[tokio::test]
async fn horse_dismounts_when_combat_or_interaction_starts() {
    let game = make_test_game_state("horse_dismount");
    let id = rider(&game).await;
    game.use_item(&id, 1).await;
    game.players
        .write()
        .await
        .get_mut(&id)
        .unwrap()
        .last_combat_at = GameState::now_ms();
    game.tick_player_movement(0.2).await;
    assert!(!game.players.read().await[&id].mounted);
    game.players
        .write()
        .await
        .get_mut(&id)
        .unwrap()
        .last_combat_at = 0;
    game.use_item(&id, 1).await;
    game.players.write().await.get_mut(&id).unwrap().object_type = Some("sit".into());
    game.tick_player_movement(0.2).await;
    assert!(!game.players.read().await[&id].mounted);
}

#[tokio::test]
async fn horse_cannot_mount_indoors_and_dismounts_on_entry() {
    use onlinerpg_shared::pathfinding::RuntimePassability;

    let game = make_test_game_state("horse_indoors");
    let id = rider(&game).await;
    game.passability_write().insert(
        "house:test".into(),
        RuntimePassability {
            house_origin_x: 10.0,
            house_origin_z: 0.0,
            min_x: 10.0,
            max_x: 14.0,
            min_z: -2.0,
            max_z: 2.0,
            floors: vec![],
            stairwells: vec![],
            yields_to_trapped_mover: false,
            is_ground: true,
        },
    );
    game.use_item(&id, 1).await;
    assert!(game.players.read().await[&id].mounted);
    game.players.write().await.get_mut(&id).unwrap().position.x = 12.0;
    game.tick_player_movement(0.2).await;
    assert!(!game.players.read().await[&id].mounted);
    game.use_item(&id, 1).await;
    assert!(!game.players.read().await[&id].mounted);
}
