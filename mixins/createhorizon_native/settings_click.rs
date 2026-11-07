fn run_createhorizon_settings_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != "createhorizon_native" {
        return;
    }
    crate::game::createhorizon_native::settings_clicked(game, button);
}
