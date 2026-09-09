use super::*;
use crate::game_state::fishing::{FightState, FishingPhase};
use onlinerpg_shared::fishing::CATCH_XP_PER_RARITY_SQ;

/// Put a hooked session one tick from landing: fish exhausted, reeled to the
/// line floor, the angler cranking. `bold_run_ms` sets the bonus chance and
/// `roll` pins the die, so the outcome is deterministic.
async fn hook_and_exhaust(
    game_state: &GameState,
    id: &PlayerId,
    rx: &mut DirectRx,
    species: &str,
    bold_run_ms: f32,
    roll: f32,
) {
    *game_state.fishing_bonus_roll.lock().unwrap() = Some(roll);
    game_state.start_fishing(id, water_target()).await;
    advance_until_bite(game_state, rx).await;
    let mut sessions = game_state.fishing_sessions.write().await;
    let session = sessions.get_mut(id).unwrap();
    let rolled = session.rolled_fish.as_mut().unwrap();
    rolled.item_def_id = species.to_string();
    rolled.rarity = game_state
        .item_defs
        .get(species)
        .unwrap()
        .rarity_tier
        .unwrap_or(0);
    let floor = session.reel_floor_m;
    session.phase = FishingPhase::Fight {
        state: FightState {
            stance: FishingAction::Reel,
            fish_state: FishState::Exhausted,
            state_ms_left: 0.0,
            tension: 10.0,
            run_ms: 8_000.0,
            bold_run_ms,
            stamina: 0.0,
            distance: floor,
            min_distance: floor,
            dir_x: -1.0,
            dir_z: 0.0,
            elapsed_ms: 12_000.0,
        },
        last_tick: tokio::time::Instant::now(),
    };
}

const ALL_BOLD_MS: f32 = 8_000.0;
const ALL_COLD_MS: f32 = 0.0;
/// A die that succeeds against any non-zero chance, and one that fails
/// against anything under the cap.
const LUCKY_ROLL: f32 = 0.0;
const UNLUCKY_ROLL: f32 = 0.99;

async fn land_with_messages(
    game_state: &GameState,
    rx: &mut DirectRx,
) -> (FishingOutcome, Vec<ServerMessage>) {
    advance(Duration::from_millis(250)).await;
    game_state.tick_fishing(None).await;
    let msgs = drain(rx);
    let outcome = msgs
        .iter()
        .find_map(|m| match m {
            ServerMessage::FishingEnded { outcome, .. } => Some(outcome.clone()),
            _ => None,
        })
        .expect("the exhausted fish at the floor lands this tick");
    (outcome, msgs)
}

async fn land(game_state: &GameState, rx: &mut DirectRx) -> FishingOutcome {
    land_with_messages(game_state, rx).await.0
}

async fn bag_count(game_state: &GameState, id: &PlayerId, species: &str) -> u32 {
    game_state
        .get_player_inventory(id)
        .await
        .unwrap()
        .bag
        .iter()
        .filter(|i| i.item_def_id == species)
        .map(|i| i.quantity)
        .sum()
}

#[tokio::test(start_paused = true)]
async fn a_fight_held_bold_throughout_can_land_two_fish() {
    let game_state = make_test_game_state("fishing_bold_two");
    let (id, mut rx) = make_angler(&game_state, "angler_bold").await;
    hook_and_exhaust(
        &game_state,
        &id,
        &mut rx,
        "raw_trout",
        ALL_BOLD_MS,
        LUCKY_ROLL,
    )
    .await;

    let outcome = land(&game_state, &mut rx).await;

    assert!(
        matches!(
            outcome,
            FishingOutcome::Caught {
                bonus_fish: true,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(bag_count(&game_state, &id, "raw_trout").await, 2);
}

/// With no bold time the chance is exactly zero, so even a lucky die pays
/// nothing.
#[tokio::test(start_paused = true)]
async fn a_fight_never_held_bold_lands_one_fish() {
    let game_state = make_test_game_state("fishing_bold_none");
    let (id, mut rx) = make_angler(&game_state, "angler_cold").await;
    hook_and_exhaust(
        &game_state,
        &id,
        &mut rx,
        "raw_trout",
        ALL_COLD_MS,
        LUCKY_ROLL,
    )
    .await;

    let outcome = land(&game_state, &mut rx).await;

    assert!(
        matches!(
            outcome,
            FishingOutcome::Caught {
                bonus_fish: false,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(bag_count(&game_state, &id, "raw_trout").await, 1);
}

/// The bonus is a roll, not a guarantee: a perfect fight with an unlucky die
/// lands one fish, and the beat before it advertised no more than the cap.
#[tokio::test(start_paused = true)]
async fn a_bold_fight_with_an_unlucky_roll_lands_one_fish() {
    let game_state = make_test_game_state("fishing_bold_unlucky");
    let (id, mut rx) = make_angler(&game_state, "angler_unlucky").await;
    hook_and_exhaust(
        &game_state,
        &id,
        &mut rx,
        "raw_trout",
        ALL_BOLD_MS,
        UNLUCKY_ROLL,
    )
    .await;

    let outcome = land(&game_state, &mut rx).await;

    assert!(
        matches!(
            outcome,
            FishingOutcome::Caught {
                bonus_fish: false,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(bag_count(&game_state, &id, "raw_trout").await, 1);
}

/// A second boot does not take the trailing hook: junk and coin pouches
/// never double, and the outcome says no bonus so the client stays honest.
#[tokio::test(start_paused = true)]
async fn junk_never_doubles_however_bold_the_fight() {
    let game_state = make_test_game_state("fishing_bold_junk");
    let (id, mut rx) = make_angler(&game_state, "angler_booted").await;
    hook_and_exhaust(
        &game_state,
        &id,
        &mut rx,
        "old_boot",
        ALL_BOLD_MS,
        LUCKY_ROLL,
    )
    .await;

    let outcome = land(&game_state, &mut rx).await;

    assert!(
        matches!(
            outcome,
            FishingOutcome::Caught {
                bonus_fish: false,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(bag_count(&game_state, &id, "old_boot").await, 1);
}

/// The bonus fish is a gift, not a second catch: skill XP is granted once.
#[tokio::test(start_paused = true)]
async fn the_bonus_fish_grants_no_extra_xp() {
    let game_state = make_test_game_state("fishing_bold_xp");
    let (id, mut rx) = make_angler(&game_state, "angler_xp").await;
    hook_and_exhaust(
        &game_state,
        &id,
        &mut rx,
        "raw_trout",
        ALL_BOLD_MS,
        LUCKY_ROLL,
    )
    .await;

    let (outcome, msgs) = land_with_messages(&game_state, &mut rx).await;
    assert!(matches!(
        outcome,
        FishingOutcome::Caught {
            bonus_fish: true,
            ..
        }
    ));

    let xp_gains: Vec<u64> = msgs
        .iter()
        .filter_map(|m| match m {
            ServerMessage::SkillXpGained { total_xp, .. } => Some(*total_xp),
            _ => None,
        })
        .collect();
    let trout_rarity: u64 = 3;
    assert_eq!(
        xp_gains,
        vec![CATCH_XP_PER_RARITY_SQ * trout_rarity * trout_rarity]
    );
}

/// Through the real tick loop, the cautious policy (line out the moment a
/// run starts) never builds bold time: every beat reports a zero bonus chance
/// and even a lucky die lands one fish.
#[tokio::test(start_paused = true)]
async fn the_cautious_policy_reports_no_bonus_chance_and_lands_one() {
    let game_state = make_test_game_state("fishing_bold_cautious");
    let (id, mut rx) = make_angler(&game_state, "angler_book").await;
    *game_state.fishing_bonus_roll.lock().unwrap() = Some(LUCKY_ROLL);
    game_state.start_fishing(&id, water_target()).await;
    advance_until_bite(&game_state, &mut rx).await;
    game_state
        .fishing_sessions
        .write()
        .await
        .get_mut(&id)
        .unwrap()
        .rolled_fish
        .as_mut()
        .unwrap()
        .item_def_id = "raw_perch".to_string();
    game_state.respond_fishing(&id, FishingAction::Hook).await;

    let (outcome, msgs) =
        flow_tests::fight_to_the_end(&game_state, &id, &mut rx, auto_stance).await;

    assert!(
        matches!(
            outcome,
            FishingOutcome::Caught {
                bonus_fish: false,
                ..
            }
        ),
        "{outcome:?}"
    );
    assert!(msgs.iter().all(|m| !matches!(
        m,
        ServerMessage::FishingFight { bonus_chance_pct, .. } if *bonus_chance_pct > 0
    )));
    assert_eq!(bag_count(&game_state, &id, "raw_perch").await, 1);
}
