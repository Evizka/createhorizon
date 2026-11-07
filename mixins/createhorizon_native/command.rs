fn run_createhorizon_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let mut words = command.split_whitespace();
    if words.next() != Some("createhorizon") || !game.native_on("createhorizon_native") {
        return;
    }

    *ran = true;
    let response = match words.next().unwrap_or("help") {
        "help" => String::from(
            "Create Horizon: the crank key toggles the nearest hand crank, the clutch key opens and closes every clutch, and /createhorizon scan reports the kinetics around you.",
        ),
        "scan" => crate::game::createhorizon_native::scan_message(game),
        _ => String::from("Usage: /createhorizon [help|scan]"),
    };
    game.native_chat(&response);
}
