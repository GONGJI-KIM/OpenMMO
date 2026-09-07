use super::*;
use onlinerpg_shared::{tree_format::TREE_V1_MAGIC, worldgen::vegetation::GRASS_V3_MAGIC};
use onlinerpg_terrain::land::{plot_addr, LandGrade, REGION_PLOTS};

fn tile_position(world: f32) -> u16 {
    (((world + 32.0) / 64.0) * 65535.0).round() as u16
}

fn vegetation(magic: u32, type_count: usize, x: f32, z: f32) -> Vec<u8> {
    let mut data = magic.to_le_bytes().to_vec();
    data.extend_from_slice(&1u32.to_le_bytes());
    for _ in 1..type_count {
        data.extend_from_slice(&0u32.to_le_bytes());
    }
    data.extend_from_slice(&tile_position(x).to_le_bytes());
    data.extend_from_slice(&tile_position(z).to_le_bytes());
    data.extend_from_slice(&[0, 0]);
    data
}

async fn builder() -> (GameState, crate::auth::AuthService, i64, DirectRx) {
    let game = make_test_game_state("house_scroll_build");
    let auth = make_test_auth("house_scroll_build");
    let account = auth.login_google("house-builder").unwrap();
    let character = create_test_character(&auth, &account, "Builder");
    let mut player = make_player("Builder", 5.0, 5.0);
    player.position.y = 5.0;
    player.level = 10;
    game.add_player(player).await;
    game.register_player_character(
        &pid("Builder"),
        character.id,
        onlinerpg_shared::xp::xp_for_level(10),
        attrs_with_cha(12),
        0,
        None,
    )
    .await;
    game.inventories.write().await.insert(
        pid("Builder"),
        PlayerInventory {
            bag: vec![bag_item(1, "land_deed", 1)],
            ..Default::default()
        },
    );
    game.terrain_io
        .write_land_grades(0, 0, &vec![LandGrade::Homestead as u8; REGION_PLOTS])
        .await
        .unwrap();
    let rx = game.register_direct_channel(&pid("Builder")).await;
    game.claim_land(
        &pid("Builder"),
        1,
        super::super::land::plot_key(plot_addr(5.0, 5.0)),
        &auth,
    )
    .await;
    (game, auth, character.id, rx)
}

#[tokio::test]
async fn house_scroll_builds_only_inside_the_owned_estate_and_persists_consumption() {
    let (game, auth, character_id, mut rx) = builder().await;
    drain(&mut rx);
    game.inventories
        .write()
        .await
        .get_mut(&pid("Builder"))
        .unwrap()
        .bag = vec![
        bag_item(2, "scroll_of_small_house", 1),
        bag_item(4, onlinerpg_shared::landscaping::TOOLBOX_ITEM, 1),
    ];
    let grass = vegetation(GRASS_V3_MAGIC, 3, 6.0, 6.0);
    game.terrain_io.write_grass(0, 0, &grass).await.unwrap();
    game.terrain_io
        .write_trees(0, 0, &vegetation(TREE_V1_MAGIC, 2, 6.0, 6.0))
        .await
        .unwrap();

    assert!(
        game.try_start_house_placement(&pid("Builder"), 2, &auth)
            .await
    );
    let messages = drain(&mut rx);
    assert!(messages.iter().any(|message| matches!(
        message,
        ServerMessage::LandscapingMode {
            tool: onlinerpg_shared::landscaping::LandscapingTool::House,
            ..
        }
    )));
    assert!(messages.iter().any(|message| matches!(
        message,
        ServerMessage::HousePlacementStarted { instance_id: 2, plots, .. }
            if plots.len() == 1
    )));

    game.place_house(
        &pid("Builder"),
        2,
        Position {
            x: 5.0,
            y: -100.0,
            z: 10.0,
        },
        1,
        &auth,
    )
    .await;
    let houses = game.housing_io.read_all_houses().await.unwrap();
    assert_eq!(houses.len(), 1);
    assert_eq!(houses[0].owner_id, character_id.to_string());
    assert_eq!(
        houses[0].source_scroll_id.as_deref(),
        Some("scroll_of_small_house")
    );
    assert_eq!(
        (houses[0].rooms[0].size_x, houses[0].rooms[0].size_z),
        (4, 6)
    );
    assert_eq!(houses[0].origin.y, 5.0);
    assert!(!game.inventories.read().await[&pid("Builder")]
        .bag
        .iter()
        .any(|item| item.item_def_id == "scroll_of_small_house"));
    let messages = drain(&mut rx);
    assert!(messages
        .iter()
        .any(|message| matches!(message, ServerMessage::HousePlacementResult { error: None })));
    assert!(messages.iter().any(|message| matches!(
        message,
        ServerMessage::TreeTilesInvalidated { tiles } if tiles == &vec![(0, 0)]
    )));
    assert!(messages.iter().any(|message| matches!(
        message,
        ServerMessage::GrassTilesInvalidated { tiles } if tiles == &vec![(0, 0)]
    )));
    let cleared_grass = game.terrain_io.read_grass(0, 0).await.unwrap().unwrap();
    let cleared_trees = game.terrain_io.read_trees(0, 0).await.unwrap().unwrap();
    assert_eq!(
        u32::from_le_bytes(cleared_grass[4..8].try_into().unwrap()),
        0
    );
    assert_eq!(
        u32::from_le_bytes(cleared_trees[4..8].try_into().unwrap()),
        0
    );
    assert_eq!(
        game.terrain_io
            .read_original_grass(0, 0)
            .await
            .unwrap()
            .unwrap(),
        grass
    );

    game.load_player_inventory(&pid("Builder"), character_id, &auth)
        .await;
    assert_eq!(game.inventories.read().await[&pid("Builder")].bag.len(), 1);
    game.inventories
        .write()
        .await
        .get_mut(&pid("Builder"))
        .unwrap()
        .bag
        .push(bag_item(3, "scroll_of_small_house", 1));
    game.place_house(
        &pid("Builder"),
        3,
        Position {
            x: 30.0,
            y: 5.0,
            z: 5.0,
        },
        0,
        &auth,
    )
    .await;
    assert_eq!(game.housing_io.read_all_houses().await.unwrap().len(), 1);
    assert_eq!(game.inventories.read().await[&pid("Builder")].bag.len(), 2);
    assert!(drain(&mut rx).iter().any(|message| matches!(
        message,
        ServerMessage::HousePlacementResult { error: Some(reason) }
            if reason.contains("whole house")
    )));

    let house_id = houses[0].id.clone();
    let scrolls_before = game.inventories.read().await[&pid("Builder")]
        .bag
        .iter()
        .filter(|item| item.item_def_id == "scroll_of_small_house")
        .count();
    game.demolish_house(&pid("Builder"), house_id.clone(), &auth)
        .await;
    assert!(game.housing_io.read_all_houses().await.unwrap().is_empty());
    let scrolls_after = game.inventories.read().await[&pid("Builder")]
        .bag
        .iter()
        .filter(|item| item.item_def_id == "scroll_of_small_house")
        .count();
    assert_eq!(scrolls_after, scrolls_before + 1);
    assert_eq!(
        auth.load_inventory(character_id)
            .unwrap()
            .iter()
            .filter(|item| item.item_def_id == "scroll_of_small_house")
            .count(),
        scrolls_before + 1
    );
    let messages = drain(&mut rx);
    assert!(messages.iter().any(|message| matches!(
        message,
        ServerMessage::HouseRemoved { house_id: removed } if removed == &house_id
    )));
    assert!(messages.iter().any(|message| matches!(
        message,
        ServerMessage::HouseDemolitionResult { house_id: removed, error: None }
            if removed == &house_id
    )));
}

#[tokio::test]
async fn house_demolition_keeps_house_when_the_scroll_is_too_heavy() {
    let (game, auth, _, mut rx) = builder().await;
    game.inventories
        .write()
        .await
        .get_mut(&pid("Builder"))
        .unwrap()
        .bag = vec![
        bag_item(2, "scroll_of_small_house", 1),
        bag_item(4, onlinerpg_shared::landscaping::TOOLBOX_ITEM, 1),
    ];
    game.place_house(
        &pid("Builder"),
        2,
        Position {
            x: 5.0,
            y: 5.0,
            z: 5.0,
        },
        0,
        &auth,
    )
    .await;
    let house_id = game.housing_io.read_all_houses().await.unwrap()[0]
        .id
        .clone();
    game.inventories
        .write()
        .await
        .get_mut(&pid("Builder"))
        .unwrap()
        .bag
        .push(bag_item(5, "stone_hearth", 1));
    drain(&mut rx);

    game.demolish_house(&pid("Builder"), house_id.clone(), &auth)
        .await;

    assert_eq!(game.housing_io.read_all_houses().await.unwrap().len(), 1);
    assert!(drain(&mut rx).iter().any(|message| matches!(
        message,
        ServerMessage::HouseDemolitionResult { house_id: rejected, error: Some(reason) }
            if rejected == &house_id && reason.contains("too heavy")
    )));
}

#[tokio::test]
async fn house_placement_requires_the_toolbox_on_the_authoritative_request() {
    let (game, auth, _, mut rx) = builder().await;
    game.inventories
        .write()
        .await
        .get_mut(&pid("Builder"))
        .unwrap()
        .bag = vec![bag_item(2, "scroll_of_small_house", 1)];

    game.place_house(
        &pid("Builder"),
        2,
        Position {
            x: 5.0,
            y: 5.0,
            z: 5.0,
        },
        0,
        &auth,
    )
    .await;

    assert!(game.housing_io.read_all_houses().await.unwrap().is_empty());
    assert!(drain(&mut rx).iter().any(|message| matches!(
        message,
        ServerMessage::HousePlacementResult { error: Some(reason) }
            if reason.contains("Toolbox")
    )));
}

#[tokio::test]
async fn house_demolition_rejects_a_non_owner() {
    let (game, auth, _, mut builder_rx) = builder().await;
    game.inventories
        .write()
        .await
        .get_mut(&pid("Builder"))
        .unwrap()
        .bag = vec![
        bag_item(2, "scroll_of_small_house", 1),
        bag_item(4, onlinerpg_shared::landscaping::TOOLBOX_ITEM, 1),
    ];
    game.place_house(
        &pid("Builder"),
        2,
        Position {
            x: 5.0,
            y: 5.0,
            z: 5.0,
        },
        0,
        &auth,
    )
    .await;
    drain(&mut builder_rx);
    let house_id = game.housing_io.read_all_houses().await.unwrap()[0]
        .id
        .clone();

    let account = auth.login_google("house-intruder").unwrap();
    let character = create_test_character(&auth, &account, "Intruder");
    game.add_player(make_player("Intruder", 5.0, 5.0)).await;
    game.register_player_character(
        &pid("Intruder"),
        character.id,
        onlinerpg_shared::xp::xp_for_level(10),
        attrs_with_cha(12),
        0,
        None,
    )
    .await;
    game.inventories.write().await.insert(
        pid("Intruder"),
        PlayerInventory {
            bag: vec![bag_item(8, onlinerpg_shared::landscaping::TOOLBOX_ITEM, 1)],
            ..Default::default()
        },
    );
    let mut rx = game.register_direct_channel(&pid("Intruder")).await;
    game.demolish_house(&pid("Intruder"), house_id.clone(), &auth)
        .await;

    assert_eq!(game.housing_io.read_all_houses().await.unwrap().len(), 1);
    assert!(drain(&mut rx).iter().any(|message| matches!(
        message,
        ServerMessage::HouseDemolitionResult { house_id: rejected, error: Some(reason) }
            if rejected == &house_id && reason.contains("own house")
    )));
}
