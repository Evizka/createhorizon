//! A model of `horizon-client`'s `game` module, with the bundle inside it.
//!
//! `createhorizon_native` is the module the launcher injects; `injected`
//! holds the four helper files, which the launcher splices into the heads of
//! Horizon's hook functions. Both are the repository's own sources, read
//! with `include!` so a change to them shows up here at once.
//!
//! The bundle's callers are Horizon's hook functions, which do not exist
//! here, so unused items are expected in a stand-in.
#![allow(dead_code)]

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use horizon_client::blocks::Fluid;

/// `horizon_world::registry::BlockStateId`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockStateId(pub u32);

/// The fields of `horizon_items::blocks::StateFacts` the mod reads.
#[derive(Clone, Debug)]
pub struct StateFacts {
    pub name: Arc<str>,
    pub fluid: Fluid,
    /// 1 through 8, 8 being a source or a falling column; 0 for no fluid.
    pub fluid_level: u8,
    pub fluid_falling: bool,
}

/// `horizon_items::blocks::BlockFacts`, indexed by block state id.
pub struct BlockFacts {
    states: Vec<StateFacts>,
}

impl BlockFacts {
    #[must_use]
    pub fn get(&self, id: BlockStateId) -> &StateFacts {
        &self.states[usize::try_from(id.0).expect("state id fits usize")]
    }
}

/// The `Resources` of `horizon-client`'s `game` module: `facts` only, the
/// one table the mod reads. It is private there, as here, and the module
/// below reaches it as a child module does.
pub struct Resources {
    facts: BlockFacts,
}

impl Resources {
    fn facts(&self) -> &BlockFacts {
        &self.facts
    }
}

/// `horizon_client_render::chunk::ColumnLayout`.
#[derive(Clone, Copy, Debug)]
pub struct ColumnLayout {
    min_y: i32,
    sections: usize,
}

impl ColumnLayout {
    #[must_use]
    pub fn min_y(self) -> i32 {
        self.min_y
    }

    #[must_use]
    pub fn max_y(self) -> i32 {
        self.min_y + i32::try_from(self.sections * 16).expect("world height fits i32")
    }
}

/// `horizon_client_render::world::World`, as the mod reads it: blocks by
/// position, whether a column is resident, and the world's height.
pub struct World {
    blocks: BTreeMap<(i32, i32, i32), BlockStateId>,
    layout: ColumnLayout,
    air: BlockStateId,
}

impl World {
    #[must_use]
    pub fn block(&self, x: i32, y: i32, z: i32) -> BlockStateId {
        self.blocks.get(&(x, y, z)).copied().unwrap_or(self.air)
    }

    #[must_use]
    pub fn loaded(&self, _x: i32, _z: i32) -> bool {
        true
    }

    #[must_use]
    pub fn layout(&self) -> ColumnLayout {
        self.layout
    }
}

/// `horizon-client`'s `NativePlayer`, as `native_player` answers it.
#[derive(Clone, Debug, PartialEq)]
pub struct NativePlayer {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub dimension: &'static str,
}

/// `horizon_client::extras::Corner`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// `horizon_client::hud::HudBatch`: the mod only draws into one.
#[derive(Default)]
pub struct HudBatch;

/// `horizon_client::extras_hud::Corners`, whose `place` stacks boxes.
#[derive(Default)]
pub struct Corners;

impl Corners {
    #[must_use]
    pub fn place(&mut self, _corner: Corner, width: f32, height: f32) -> (f32, f32) {
        (0.0 + width.min(1.0), 0.0 + height.min(1.0))
    }
}

/// `horizon-client`'s `NativeMod`, as `native_mods` collects it.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeMod {
    pub id: String,
    pub title: String,
    pub description: String,
    pub keys: Vec<[String; 3]>,
}

/// The `Game` of `horizon-client`, carrying the hooks and the helpers the
/// mod uses. Everything past `world` is the harness's own fixture state.
pub struct Game<'a> {
    world: World,
    res: Arc<Resources>,
    player: Option<NativePlayer>,
    enabled: bool,
    playing: bool,
    keys: BTreeMap<String, String>,
    chat: Vec<String>,
    /// What the HUD drew: filled boxes and lines of text.
    fills: Cell<usize>,
    texts: Cell<usize>,
    lifetime: std::marker::PhantomData<&'a ()>,
}

impl Game<'_> {
    /// Whether the player left mod `mod_id` on in Options > Mods.
    #[must_use]
    pub fn native_on(&self, mod_id: &str) -> bool {
        mod_id == "createhorizon_native" && self.enabled
    }

    #[must_use]
    pub fn native_playing(&self) -> bool {
        self.playing
    }

    #[must_use]
    pub fn native_window(&self) -> bool {
        true
    }

    #[must_use]
    pub fn native_corner_shown(&self, _corner: Corner) -> bool {
        true
    }

    #[must_use]
    pub fn native_text_width(&self, text: &str) -> f32 {
        text.chars().count() as f32 * 6.0
    }

    pub fn native_fill(&self, _batch: &mut HudBatch, _rect: [f32; 4], _argb: u32) {
        self.fills.set(self.fills.get() + 1);
    }

    pub fn native_text(
        &self,
        _batch: &mut HudBatch,
        _text: &str,
        _at: [f32; 2],
        _rgb: u32,
        _shadow: bool,
    ) {
        self.texts.set(self.texts.get() + 1);
    }

    #[must_use]
    pub fn native_player(&self) -> Option<NativePlayer> {
        self.player.clone()
    }

    #[must_use]
    pub fn native_mod_key(&self, _mod_id: &str, name: &str, default: &str) -> String {
        self.keys
            .get(name)
            .cloned()
            .unwrap_or_else(|| default.to_owned())
    }

    pub fn native_chat(&mut self, text: &str) {
        self.chat.push(text.to_owned());
    }
}

impl Game<'_> {
    /// Builds a game over a world, with the mod on and the player in it.
    #[must_use]
    pub fn new(world: World, resources: Arc<Resources>, player: NativePlayer) -> Self {
        Self {
            world,
            res: resources,
            player: Some(player),
            enabled: true,
            playing: true,
            keys: BTreeMap::new(),
            chat: Vec::new(),
            fills: Cell::new(0),
            texts: Cell::new(0),
            lifetime: std::marker::PhantomData,
        }
    }

    /// Turns the mod off, as Options > Mods does.
    pub fn with_mod_disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Rebinds one of the mod's keys.
    pub fn with_key(mut self, name: &str, key: &str) -> Self {
        self.keys.insert(name.to_owned(), key.to_owned());
        self
    }

    /// The lines the mod has put in chat, oldest first.
    #[must_use]
    pub fn chat(&self) -> &[String] {
        &self.chat
    }

    /// How many boxes and how many lines of text the HUD has drawn.
    #[must_use]
    pub fn draws(&self) -> (usize, usize) {
        (self.fills.get(), self.texts.get())
    }
}

/// The module the launcher injects into `crates/horizon-client/src/game.rs`.
pub mod createhorizon_native {
    include!("../../mixins/createhorizon_native/runtime.rs");
}

/// The helper files, which the launcher splices into the heads of the hook
/// functions. Nested items there, these are module-level items here.
pub mod injected {
    use super::{Corner, Corners, Game, HudBatch, NativeMod};

    include!("../../mixins/createhorizon_native/command.rs");
    include!("../../mixins/createhorizon_native/hud.rs");
    include!("../../mixins/createhorizon_native/key.rs");
    include!("../../mixins/createhorizon_native/mods.rs");
    include!("../../mixins/createhorizon_native/settings.rs");
    include!("../../mixins/createhorizon_native/settings_click.rs");

    /// The tests of the helpers, and of the runtime behind them. They live
    /// here because the helpers are private, as they are nested in Horizon's
    /// hook functions.
    #[cfg(test)]
    mod tests;
}

/// What a test puts in a world: a named block, or water of a level.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum BlockSpec {
    Named(&'static str),
    Water { level: u8, falling: bool },
}

impl BlockSpec {
    fn facts(&self) -> StateFacts {
        match *self {
            Self::Named(name) => StateFacts {
                name: Arc::from(name),
                fluid: Fluid::None,
                fluid_level: 0,
                fluid_falling: false,
            },
            Self::Water { level, falling } => StateFacts {
                name: Arc::from("minecraft:water"),
                fluid: Fluid::Water,
                fluid_level: level,
                fluid_falling: falling,
            },
        }
    }
}

/// A world built one block at a time, for the tests below.
#[derive(Default)]
pub struct TestWorld {
    placed: BTreeMap<(i32, i32, i32), BlockSpec>,
}

impl TestWorld {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, name: &'static str) {
        self.placed.insert((x, y, z), BlockSpec::Named(name));
    }

    /// Water at a position: `level` 8 is a source or a falling column.
    pub fn water(&mut self, x: i32, y: i32, z: i32, level: u8, falling: bool) {
        self.placed
            .insert((x, y, z), BlockSpec::Water { level, falling });
    }

    /// The game with this world loaded and the player at `x, y, z`.
    #[must_use]
    pub fn game(&self, x: f64, y: f64, z: f64) -> Game<'static> {
        let air = BlockSpec::Named("minecraft:air");
        let mut states = vec![air.facts()];
        let mut ids: BTreeMap<&BlockSpec, BlockStateId> = BTreeMap::new();
        for spec in self.placed.values().collect::<BTreeSet<_>>() {
            let id = BlockStateId(u32::try_from(states.len()).expect("few states"));
            states.push(spec.facts());
            ids.insert(spec, id);
        }
        let blocks = self
            .placed
            .iter()
            .map(|(position, spec)| (*position, ids[spec]))
            .collect();
        let world = World {
            blocks,
            layout: ColumnLayout {
                min_y: -64,
                sections: 24,
            },
            air: BlockStateId(0),
        };
        let resources = Arc::new(Resources {
            facts: BlockFacts { states },
        });
        Game::new(
            world,
            resources,
            NativePlayer {
                x,
                y,
                z,
                yaw: 0.0,
                pitch: 0.0,
                dimension: "minecraft:overworld",
            },
        )
    }
}
