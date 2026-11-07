//! Tests of the injected helpers and of the runtime behind them.
//!
//! This module lives inside [`crate::game::injected`] because the helper
//! files it exercises are private there, exactly as they are nested in
//! Horizon's hook functions.

use super::*;
use crate::game::{Corners, TestWorld, createhorizon_native};
use std::sync::{Mutex, MutexGuard};

use create_engine::KINETICS_RULES_VERSION;

/// The runtime keeps its toggles in process-wide atomics, so the tests
/// that move them take this lock and start from a known state.
static TOGGLES: Mutex<()> = Mutex::new(());

fn toggles() -> MutexGuard<'static, ()> {
    let guard = TOGGLES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    createhorizon_native::reset_for_tests();
    guard
}

/// The five-part network the README asks a player to build.
fn sample_network() -> TestWorld {
    let mut world = TestWorld::new();
    world.set(0, 0, 0, "createhorizon:hand_crank");
    world.set(0, 0, 1, "createhorizon:shaft");
    world.set(0, 0, 2, "createhorizon:large_cogwheel");
    world.set(1, 1, 2, "createhorizon:small_cogwheel");
    world.set(0, 0, -1, "createhorizon:millstone");
    world
}

fn scan(game: &mut Game<'_>) -> String {
    createhorizon_native::scan_message(game)
}

#[test]
fn the_sample_network_scans_as_the_readme_says() {
    let _guard = toggles();
    let mut game = sample_network().game(0.5, 0.5, 0.5);
    run_createhorizon_key(&mut game, "key.keyboard.c", true);
    let message = scan(&mut game);
    assert!(message.contains("crank=on"), "{message}");
    assert!(message.contains("parts=5"), "{message}");
    assert!(message.contains("links=4"), "{message}");
    assert!(message.contains("networks=1"), "{message}");
    assert!(message.contains("driven=5"), "{message}");
    assert!(message.contains("max=32 rpm"), "{message}");
    assert!(message.contains("overload=0"), "{message}");
}

#[test]
fn a_crushing_wheel_instead_of_a_millstone_overloads_the_network() {
    let _guard = toggles();
    let mut world = sample_network();
    world.set(0, 0, -1, "createhorizon:crushing_wheel");
    let mut game = world.game(0.5, 0.5, 0.5);
    run_createhorizon_key(&mut game, "key.keyboard.c", true);
    let message = scan(&mut game);
    assert!(message.contains("overload=1"), "{message}");
    assert!(message.contains("driven=0"), "{message}");
    assert!(message.contains("max=0 rpm"), "{message}");
}

#[test]
fn the_crank_key_toggles_and_the_command_reports_it() {
    let _guard = toggles();
    let mut game = sample_network().game(0.5, 0.5, 0.5);
    assert!(scan(&mut game).contains("crank=off"));
    run_createhorizon_key(&mut game, "key.keyboard.c", true);
    assert!(scan(&mut game).contains("crank=on"));
    run_createhorizon_key(&mut game, "key.keyboard.c", true);
    assert!(scan(&mut game).contains("crank=off"));
}

#[test]
fn the_command_runs_only_for_its_own_words_and_while_the_mod_is_on() {
    let _guard = toggles();
    let mut game = sample_network().game(0.5, 0.5, 0.5);
    let mut ran = false;
    run_createhorizon_command(&mut game, "notcreatehorizon scan", &mut ran);
    assert!(!ran);

    run_createhorizon_command(&mut game, "createhorizon scan", &mut ran);
    assert!(ran);
    assert_eq!(game.chat().len(), 1);
    assert!(
        game.chat()[0].starts_with("Create scan:"),
        "{}",
        game.chat()[0]
    );

    let mut off = sample_network().game(0.5, 0.5, 0.5).with_mod_disabled();
    let mut ran = false;
    run_createhorizon_command(&mut off, "createhorizon scan", &mut ran);
    assert!(!ran);
}

#[test]
fn the_help_and_usage_lines_answer_in_chat() {
    let mut game = sample_network().game(0.5, 0.5, 0.5);
    let mut ran = false;
    run_createhorizon_command(&mut game, "createhorizon", &mut ran);
    run_createhorizon_command(&mut game, "createhorizon nonsense", &mut ran);
    assert_eq!(game.chat().len(), 2);
    assert!(
        game.chat()[0].starts_with("Create Horizon:"),
        "{}",
        game.chat()[0]
    );
    assert_eq!(game.chat()[1], "Usage: /createhorizon [help|scan]");
}

#[test]
fn a_wheel_reads_the_water_beside_it() {
    let _guard = toggles();
    let mut world = TestWorld::new();
    world.set(0, 0, 0, "createhorizon:water_wheel");
    world.set(0, 0, 1, "createhorizon:shaft");
    // Two sources and a falling column beside the wheel: 8 + 8 + 12,
    // clamped to the wheel's full flow of 16.
    world.water(1, 0, 0, 8, false);
    world.water(-1, 0, 0, 8, false);
    world.water(0, 1, 0, 8, true);
    let mut game = world.game(0.5, 0.5, 0.5);
    let message = scan(&mut game);
    assert!(message.contains("flow=16"), "{message}");
    assert!(message.contains("max=4 rpm"), "{message}");
    assert!(message.contains("driven=2"), "{message}");
}

#[test]
fn a_wheel_without_water_is_a_passive_shaft() {
    let mut world = TestWorld::new();
    world.set(0, 0, 0, "createhorizon:water_wheel");
    world.set(0, 0, 1, "createhorizon:shaft");
    let mut game = world.game(0.5, 0.5, 0.5);
    let message = scan(&mut game);
    assert!(message.contains("flow=0"), "{message}");
    assert!(message.contains("driven=0"), "{message}");
}

#[test]
fn a_gearbox_turns_a_horizontal_run_into_a_vertical_one() {
    let _guard = toggles();
    let mut world = TestWorld::new();
    world.set(0, 0, 0, "createhorizon:hand_crank");
    world.set(0, 0, 1, "createhorizon:gearbox");
    world.set(0, 1, 1, "createhorizon:vertical_shaft");
    world.set(0, 2, 1, "createhorizon:vertical_shaft");
    let mut game = world.game(0.5, 0.5, 0.5);
    run_createhorizon_key(&mut game, "key.keyboard.c", true);
    let message = scan(&mut game);
    assert!(message.contains("parts=4"), "{message}");
    assert!(message.contains("links=3"), "{message}");
    assert!(message.contains("networks=1"), "{message}");
    assert!(message.contains("driven=4"), "{message}");
}

#[test]
fn the_clutch_key_splits_the_network_it_sits_in() {
    let _guard = toggles();
    let mut world = TestWorld::new();
    world.set(0, 0, 0, "createhorizon:hand_crank");
    world.set(0, 0, 1, "createhorizon:clutch");
    world.set(0, 0, 2, "createhorizon:shaft");
    let mut game = world.game(0.5, 0.5, 0.5);
    run_createhorizon_key(&mut game, "key.keyboard.c", true);

    let engaged = scan(&mut game);
    assert!(engaged.contains("clutch=engaged"), "{engaged}");
    assert!(engaged.contains("driven=3"), "{engaged}");

    run_createhorizon_key(&mut game, "key.keyboard.v", true);
    let released = scan(&mut game);
    assert!(released.contains("clutch=released"), "{released}");
    assert!(released.contains("driven=1"), "{released}");
}

#[test]
fn the_gauge_reports_the_speed_where_it_stands() {
    let _guard = toggles();
    let mut world = sample_network();
    // On the small cogwheel's own axle, so the two mesh: the gauge is a
    // shaft-like part about Z, and it reads that network's -32 rpm.
    world.set(1, 1, 3, "createhorizon:speedometer");
    let mut game = world.game(0.5, 0.5, 0.5);
    run_createhorizon_key(&mut game, "key.keyboard.c", true);
    let message = scan(&mut game);
    assert!(message.contains("gauge=-32 rpm"), "{message}");
    assert!(message.contains("driven=6"), "{message}");
}

#[test]
fn the_hud_reports_the_network_and_the_crank() {
    let _guard = toggles();
    let mut game = sample_network().game(0.5, 0.5, 0.5);
    let mut batch = HudBatch;
    let mut corners = Corners;
    draw_createhorizon_hud(&mut game, &mut batch, &mut corners);
    // One box, one line of text per HUD line.
    assert_eq!(game.draws(), (1, 5));
    let lines = createhorizon_native::hud_lines(&game);
    assert_eq!(lines.len(), 5);
    assert!(
        lines[0].contains(&format!("kinetics {KINETICS_RULES_VERSION}")),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].contains("5 parts, 4 links, 1 networks"),
        "{}",
        lines[1]
    );
    assert!(lines[2].contains("0 driven at 0 rpm"), "{}", lines[2]);
    assert!(lines[3].contains("press C to start"), "{}", lines[3]);
    assert_eq!(lines[4], "Clutch: engaged");
}

#[test]
fn the_mod_lists_itself_with_its_keys() {
    let mut mods: Vec<NativeMod> = Vec::new();
    register_createhorizon_mod(&mut mods);
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].id, "createhorizon_native");
    let names: Vec<&str> = mods[0].keys.iter().map(|key| key[0].as_str()).collect();
    assert!(names.contains(&"crank"), "{names:?}");
    assert!(names.contains(&"clutch"), "{names:?}");
}

#[test]
fn a_rebound_crank_key_is_the_one_the_hud_names() {
    let _guard = toggles();
    let world = sample_network();
    let mut game = world
        .game(0.5, 0.5, 0.5)
        .with_key("crank", "key.keyboard.r");
    let lines = createhorizon_native::hud_lines(&game);
    assert!(lines[3].contains("press R to start"), "{}", lines[3]);
    run_createhorizon_key(&mut game, "key.keyboard.c", true);
    let lines = createhorizon_native::hud_lines(&game);
    assert!(lines[3].contains("press R to start"), "{}", lines[3]);
    run_createhorizon_key(&mut game, "key.keyboard.r", true);
    let lines = createhorizon_native::hud_lines(&game);
    assert!(!lines[3].contains("press R to start"), "{}", lines[3]);
}

#[test]
fn a_turned_off_mod_does_not_toggle_or_draw() {
    let _guard = toggles();
    let mut game = sample_network().game(0.5, 0.5, 0.5).with_mod_disabled();
    run_createhorizon_key(&mut game, "key.keyboard.c", true);
    assert!(scan(&mut game).contains("crank=off"));
}

#[test]
fn the_settings_page_switches_the_same_toggles_as_the_keys() {
    let _guard = toggles();
    let mut game = sample_network().game(0.5, 0.5, 0.5);

    let mut buttons: Vec<String> = Vec::new();
    list_createhorizon_settings(&mut game, "createhorizon_native", &mut buttons);
    assert_eq!(buttons, ["Clutch: engaged", "Hand crank: idle"]);

    run_createhorizon_settings_click(&mut game, "createhorizon_native", 0);
    run_createhorizon_settings_click(&mut game, "createhorizon_native", 1);
    let mut after: Vec<String> = Vec::new();
    list_createhorizon_settings(&mut game, "createhorizon_native", &mut after);
    assert_eq!(after, ["Clutch: released", "Hand crank: running"]);
    assert!(scan(&mut game).contains("clutch=released"));

    // Another mod's page is left alone.
    let mut other: Vec<String> = Vec::new();
    list_createhorizon_settings(&mut game, "someone_else", &mut other);
    assert!(other.is_empty());
}

#[test]
fn the_readme_second_network_runs_on_water_through_a_gearbox() {
    let _guard = toggles();
    let mut world = TestWorld::new();
    world.set(0, 0, 0, "createhorizon:water_wheel");
    world.water(1, 0, 0, 8, false);
    world.water(-1, 0, 0, 8, false);
    world.water(0, 1, 0, 8, false);
    world.set(0, 0, 1, "createhorizon:gearbox");
    world.set(0, 1, 1, "createhorizon:vertical_shaft");
    world.set(0, 2, 1, "createhorizon:speedometer");
    world.set(0, 3, 1, "createhorizon:clutch");
    world.set(0, 4, 1, "createhorizon:vertical_shaft");
    let mut game = world.game(0.5, 0.5, 0.5);

    let engaged = scan(&mut game);
    println!("ENGAGED  {engaged}");
    run_createhorizon_key(&mut game, "key.keyboard.v", true);
    let released = scan(&mut game);
    println!("RELEASED {released}");
}
