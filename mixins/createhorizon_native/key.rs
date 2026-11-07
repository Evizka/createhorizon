fn run_createhorizon_key(game: &mut Game<'_>, key: &str, pressed: bool) {
    crate::game::createhorizon_native::key_event(game, key, pressed);
}
