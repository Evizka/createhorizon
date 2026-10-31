fn register_createhorizon_mod(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: String::from("createhorizon_native"),
        title: String::from("Create Horizon 0.1.0"),
        description: String::from(
            "Deterministic fixed-point kinetic prototype; the content pack supplies six sample components.",
        ),
        keys: Vec::new(),
    });
}
