fn draw_createhorizon_hud(game: &mut Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    if !game.native_on("createhorizon_native")
        || !game.native_window()
        || !game.native_playing()
        || !game.native_corner_shown(Corner::TopLeft)
    {
        return;
    }

    let lines = crate::game::createhorizon_native::hud_lines(game);
    let width = lines
        .iter()
        .map(|line| game.native_text_width(line))
        .fold(0.0, f32::max)
        + 4.0;
    let height = 32.0;
    let (x, y) = corners.place(Corner::TopLeft, width, height);
    game.native_fill(batch, [x, y, width, height], 0x7800_0000);
    for (index, line) in lines.iter().enumerate() {
        let line_y = match index {
            0 => y + 2.0,
            1 => y + 12.0,
            _ => y + 22.0,
        };
        game.native_text(batch, line, [x + 2.0, line_y], 0xD6E7F0, true);
    }
}
