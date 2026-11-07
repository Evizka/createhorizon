fn list_createhorizon_settings(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != "createhorizon_native" || !game.native_on(mod_id) {
        return;
    }
    buttons.extend(crate::game::createhorizon_native::settings_buttons(game));
}
