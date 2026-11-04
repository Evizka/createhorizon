fn register_createhorizon_mod(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: String::from("createhorizon_native"),
        title: String::from("Create Horizon 0.1.0"),
        description: String::from(
            "Fixed-point kinetics prototype; toggle the nearest hand crank with the configured key.",
        ),
        keys: vec![[
            String::from("crank"),
            String::from("Toggle nearest hand crank"),
            String::from("key.keyboard.c"),
        ]],
    });
}
