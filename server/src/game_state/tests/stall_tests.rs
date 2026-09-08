// ---- Stalls (/lay_stall, the peddler_stall item, consignment) -------------

use super::*;
use onlinerpg_shared::character::CharacterClass;
use onlinerpg_shared::messages::StallBuyLine;
use onlinerpg_shared::stall::STALL_LEASH_M;

#[tokio::test]
async fn only_a_merchant_lays_a_stall_and_only_one_at_a_time() {
    let game_state = make_test_game_state("stall_lay");
    let auth = make_test_auth("stall_lay");
    let knight_id = pid("knight");
    game_state
        .add_player(make_player("knight", 100.0, 50.0))
        .await;
    let merchant_id = pid("npc_wick");
    let mut merchant = make_player("npc_wick", 100.0, 50.0);
    merchant.class = CharacterClass::Merchant;
    merchant.is_official_npc = true;
    game_state.add_player(merchant).await;

    game_state
        .send_chat_message(&knight_id, "/lay_stall".to_string(), &auth)
        .await;
    assert!(
        game_state.stalls.read().await.is_empty(),
        "non-merchants are refused"
    );

    game_state
        .send_chat_message(&merchant_id, "/lay_stall".to_string(), &auth)
        .await;
    {
        let stalls = game_state.stalls.read().await;
        assert_eq!(stalls.len(), 1);
        assert_eq!(stalls.values().next().unwrap().stall.owner, merchant_id);
    }

    game_state
        .send_chat_message(&merchant_id, "/lay_stall".to_string(), &auth)
        .await;
    assert_eq!(
        game_state.stalls.read().await.len(),
        1,
        "a second stall by the same owner is refused"
    );
}

#[tokio::test]
async fn pack_stall_and_logout_both_fold_the_table() {
    let game_state = make_test_game_state("stall_pack");
    let auth = make_test_auth("stall_pack");
    let merchant_id = pid("npc_wick");
    let mut merchant = make_player("npc_wick", 100.0, 50.0);
    merchant.class = CharacterClass::Merchant;
    game_state.add_player(merchant).await;

    game_state
        .send_chat_message(&merchant_id, "/pack_stall".to_string(), &auth)
        .await;
    assert!(game_state.stalls.read().await.is_empty());

    game_state
        .send_chat_message(&merchant_id, "/lay_stall".to_string(), &auth)
        .await;
    assert_eq!(game_state.stalls.read().await.len(), 1);
    game_state
        .send_chat_message(&merchant_id, "/pack_stall".to_string(), &auth)
        .await;
    assert!(
        game_state.stalls.read().await.is_empty(),
        "/pack_stall removes the stall"
    );

    game_state
        .send_chat_message(&merchant_id, "/lay_stall".to_string(), &auth)
        .await;
    assert_eq!(game_state.stalls.read().await.len(), 1);
    game_state.remove_player(&merchant_id).await;
    assert!(
        game_state.stalls.read().await.is_empty(),
        "logout packs the stall up"
    );
}

/// A stallholder and a customer backed by real DB characters, so a sale can be
/// asserted against what actually landed on disk.
struct Market {
    game_state: GameState,
    auth: crate::auth::AuthService,
    owner: PlayerId,
    customer: PlayerId,
}

async fn make_market(test_name: &str, owner_gold: i64, customer_gold: i64) -> Market {
    let auth = make_test_auth(test_name);
    let account = auth.login_npc(&format!("npc_{test_name}")).unwrap();
    let game_state = make_test_game_state(test_name);
    for (name, x, gold) in [("Sella", 100.0, owner_gold), ("Bram", 101.0, customer_gold)] {
        let record = create_test_character(&auth, &account, name);
        let mut player = make_player(name, x, 50.0);
        player.name = name.to_string();
        game_state.add_player(player).await;
        game_state
            .register_player_character(&pid(name), record.id, 0, attrs_with_cha(12), gold, None)
            .await;
        game_state
            .inventories
            .write()
            .await
            .insert(pid(name), PlayerInventory::default());
    }
    Market {
        game_state,
        auth,
        owner: pid("Sella"),
        customer: pid("Bram"),
    }
}

async fn give(game_state: &GameState, player_id: &PlayerId, item: ItemInstance) {
    game_state
        .inventories
        .write()
        .await
        .get_mut(player_id)
        .unwrap()
        .bag
        .push(item);
}

fn buy(instance_id: u64, quantity: u32) -> Vec<StallBuyLine> {
    vec![StallBuyLine {
        instance_id,
        quantity,
    }]
}

async fn stall_id(game_state: &GameState, owner: &PlayerId) -> u64 {
    game_state.stall_of(owner).await.expect("stall out").id
}

#[tokio::test]
async fn the_item_lays_the_table_out_and_using_it_again_packs_it_up() {
    let market = make_market("stall_item", 0, 0).await;
    let game_state = &market.game_state;
    give(game_state, &market.owner, bag_item(1, "peddler_stall", 1)).await;

    game_state.use_item(&market.owner, 1).await;

    {
        let stalls = game_state.stalls.read().await;
        let entry = stalls.get(&market.owner).expect("stall laid out");
        assert_eq!(entry.stall.owner_name, "Sella");
        assert_eq!(entry.placed_with, 1);
        // Rotation 0 faces +z, so the table lands ahead of the owner.
        assert!(entry.stall.position.z > 50.0);
    }
    assert_eq!(
        game_state.inventories.read().await[&market.owner].bag.len(),
        1,
        "the stall item is not consumed"
    );

    game_state.use_item(&market.owner, 1).await;
    assert!(game_state.stalls.read().await.is_empty());
}

#[tokio::test]
async fn straying_past_the_leash_packs_the_stall_up() {
    let market = make_market("stall_leash", 0, 0).await;
    let game_state = &market.game_state;
    give(game_state, &market.owner, bag_item(1, "peddler_stall", 1)).await;
    game_state.use_item(&market.owner, 1).await;
    let table = game_state.stall_of(&market.owner).await.unwrap().position;

    let near = Position {
        x: table.x,
        y: 0.0,
        z: table.z + STALL_LEASH_M - 1.0,
    };
    game_state
        .teleport_player(&market.owner, near, 0.0, 0)
        .await;
    assert_eq!(game_state.stalls.read().await.len(), 1);

    let far = Position {
        x: table.x,
        y: 0.0,
        z: table.z + STALL_LEASH_M + 3.0,
    };
    game_state.teleport_player(&market.owner, far, 0.0, 0).await;
    assert!(
        game_state.stalls.read().await.is_empty(),
        "walking away folds the table"
    );
}

#[tokio::test]
async fn buying_off_a_stall_moves_the_goods_and_taxes_the_seller() {
    let market = make_market("stall_buy", 0, 1_000).await;
    let game_state = &market.game_state;
    give(game_state, &market.owner, bag_item(1, "peddler_stall", 1)).await;
    give(game_state, &market.owner, bag_item(2, "healing_potion", 5)).await;
    game_state.use_item(&market.owner, 1).await;
    let id = stall_id(game_state, &market.owner).await;

    game_state.list_stall_item(&market.owner, 2, 5, 100).await;
    game_state
        .buy_from_stall(&market.customer, id, buy(2, 3), &market.auth)
        .await;

    let inventories = game_state.inventories.read().await;
    let sold = inventories[&market.owner]
        .bag
        .iter()
        .find(|i| i.instance_id == 2)
        .expect("the rest stays on the shelf");
    assert_eq!(sold.quantity, 2);
    assert_eq!(
        inventories[&market.customer].bag[0].item_def_id,
        "healing_potion"
    );
    assert_eq!(inventories[&market.customer].bag[0].quantity, 3);
    drop(inventories);

    // 300 copper at 5% tax: the customer pays the tag, the seller keeps 285.
    assert_eq!(game_state.get_player_gold(&market.customer).await, 700);
    assert_eq!(game_state.get_player_gold(&market.owner).await, 285);
    assert_eq!(
        game_state.stalls.read().await[&market.owner].listings[0].quantity,
        2
    );
}

#[tokio::test]
async fn a_sold_out_listing_leaves_the_table_and_refuses_the_next_customer() {
    let market = make_market("stall_soldout", 0, 1_000).await;
    let game_state = &market.game_state;
    give(game_state, &market.owner, bag_item(1, "peddler_stall", 1)).await;
    give(game_state, &market.owner, bag_item(2, "apple", 2)).await;
    game_state.use_item(&market.owner, 1).await;
    let id = stall_id(game_state, &market.owner).await;
    game_state.list_stall_item(&market.owner, 2, 2, 10).await;

    game_state
        .buy_from_stall(&market.customer, id, buy(2, 2), &market.auth)
        .await;
    assert!(game_state.stalls.read().await[&market.owner]
        .listings
        .is_empty());

    let gold = game_state.get_player_gold(&market.customer).await;
    game_state
        .buy_from_stall(&market.customer, id, buy(2, 1), &market.auth)
        .await;
    assert_eq!(
        game_state.get_player_gold(&market.customer).await,
        gold,
        "the second customer pays nothing for goods already gone"
    );
}

#[tokio::test]
async fn listed_goods_and_the_deployed_stall_itself_are_locked() {
    let market = make_market("stall_lock", 0, 0).await;
    let game_state = &market.game_state;
    give(game_state, &market.owner, bag_item(1, "peddler_stall", 1)).await;
    give(game_state, &market.owner, bag_item(2, "apple", 4)).await;
    game_state.use_item(&market.owner, 1).await;

    game_state.list_stall_item(&market.owner, 1, 1, 500).await;
    assert!(
        game_state.stalls.read().await[&market.owner]
            .listings
            .is_empty(),
        "the table's own item cannot go on the table"
    );

    game_state.list_stall_item(&market.owner, 2, 4, 10).await;
    assert_eq!(
        game_state.stall_reserved_quantity(&market.owner, 2).await,
        4
    );
    game_state.drop_item(&market.owner, 2).await;
    assert_eq!(
        game_state.inventories.read().await[&market.owner].bag[1].quantity,
        4,
        "listed goods cannot be dropped out from under a customer"
    );

    game_state.unlist_stall_item(&market.owner, 2).await;
    assert_eq!(
        game_state.stall_reserved_quantity(&market.owner, 2).await,
        0
    );
}

/// A cart is one purchase: if any line cannot be filled, nothing moves and
/// every line goes back on the table. Matches the merchant's `BuyItems`.
#[tokio::test]
async fn a_cart_line_that_cannot_be_filled_rolls_the_whole_purchase_back() {
    let market = make_market("stall_cart", 0, 1_000).await;
    let game_state = &market.game_state;
    give(game_state, &market.owner, bag_item(1, "peddler_stall", 1)).await;
    give(game_state, &market.owner, bag_item(2, "apple", 4)).await;
    give(game_state, &market.owner, bag_item(3, "bread", 2)).await;
    game_state.use_item(&market.owner, 1).await;
    let id = stall_id(game_state, &market.owner).await;
    game_state.list_stall_item(&market.owner, 2, 4, 10).await;
    game_state.list_stall_item(&market.owner, 3, 2, 10).await;

    // Second line asks for more bread than is on the table.
    game_state
        .buy_from_stall(
            &market.customer,
            id,
            vec![
                StallBuyLine {
                    instance_id: 2,
                    quantity: 2,
                },
                StallBuyLine {
                    instance_id: 3,
                    quantity: 5,
                },
            ],
            &market.auth,
        )
        .await;

    assert_eq!(
        game_state.get_player_gold(&market.customer).await,
        1_000,
        "nothing is charged"
    );
    assert!(
        game_state.inventories.read().await[&market.customer]
            .bag
            .is_empty(),
        "no goods move"
    );
    let stalls = game_state.stalls.read().await;
    let listings = &stalls[&market.owner].listings;
    assert_eq!(listings.len(), 2, "both listings are back on the table");
    assert_eq!(
        listings
            .iter()
            .find(|l| l.instance_id == 2)
            .unwrap()
            .quantity,
        4
    );
    assert_eq!(
        listings
            .iter()
            .find(|l| l.instance_id == 3)
            .unwrap()
            .quantity,
        2
    );
}

/// Two lines, one purchase: the tax is charged once on the whole total.
#[tokio::test]
async fn a_multi_line_cart_moves_together_and_is_taxed_once() {
    let market = make_market("stall_cart_ok", 0, 1_000).await;
    let game_state = &market.game_state;
    give(game_state, &market.owner, bag_item(1, "peddler_stall", 1)).await;
    give(game_state, &market.owner, bag_item(2, "apple", 4)).await;
    give(game_state, &market.owner, bag_item(3, "bread", 2)).await;
    game_state.use_item(&market.owner, 1).await;
    let id = stall_id(game_state, &market.owner).await;
    game_state.list_stall_item(&market.owner, 2, 4, 100).await;
    game_state.list_stall_item(&market.owner, 3, 2, 100).await;

    game_state
        .buy_from_stall(
            &market.customer,
            id,
            vec![
                StallBuyLine {
                    instance_id: 2,
                    quantity: 2,
                },
                StallBuyLine {
                    instance_id: 3,
                    quantity: 2,
                },
            ],
            &market.auth,
        )
        .await;

    // 400 copper at 5%: the customer pays the tags, the seller keeps 380.
    assert_eq!(game_state.get_player_gold(&market.customer).await, 600);
    assert_eq!(game_state.get_player_gold(&market.owner).await, 380);
    assert_eq!(
        game_state.inventories.read().await[&market.customer]
            .bag
            .len(),
        2
    );
}
