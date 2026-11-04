fn run_createhorizon_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let mut words = command.split_whitespace();
    if words.next() != Some("createhorizon") || !game.native_on("createhorizon_native") {
        return;
    }

    *ran = true;
    let response = match words.next().unwrap_or("help") {
        "help" => String::from(
            "Create Horizon: press the configured crank key to toggle the nearest hand crank; /createhorizon scan reports nearby kinetics.",
        ),
        "scan" => crate::game::createhorizon_native::scan_message(game),
        _ => String::from("Usage: /createhorizon [help|scan]"),
    };
    game.native_chat(&response);
}
