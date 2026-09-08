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
