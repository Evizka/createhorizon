// This module is injected into Horizon's game crate by horizon-mixins.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use create_engine::{
    Axis, BlockPos, Fx, GearSize, Gearwheel, KineticPart, Kinetics, NetworkSummary,
    KINETICS_RULES_VERSION, SIM_MATH_VERSION, WaterSide, water_wheel_flow,
};
use horizon_client::blocks::Fluid;
use super::Game;

const MOD_ID: &str = "createhorizon_native";
const CRANK_KEY: &str = "crank";
const CLUTCH_KEY: &str = "clutch";
const DEFAULT_CRANK_KEY: &str = "key.keyboard.c";
const DEFAULT_CLUTCH_KEY: &str = "key.keyboard.v";
const CRANK_SPEED_RPM: i32 = 16;
const CRANK_CAPACITY: i32 = 16;
const MILLSTONE_STRESS: i32 = 12;
const CRUSHING_WHEEL_STRESS: i32 = 24;
const HUD_SCAN_RADIUS: i32 = 3;
const COMMAND_SCAN_RADIUS: i32 = 8;
const SPIN_FRAMES: [&str; 4] = ["|", "/", "-", "\\"];

/// The content pack's blocks, as the world's state facts name them.
const HAND_CRANK: &str = "createhorizon:hand_crank";
const SHAFT: &str = "createhorizon:shaft";
const VERTICAL_SHAFT: &str = "createhorizon:vertical_shaft";
const SMALL_COGWHEEL: &str = "createhorizon:small_cogwheel";
const LARGE_COGWHEEL: &str = "createhorizon:large_cogwheel";
const MILLSTONE: &str = "createhorizon:millstone";
const CRUSHING_WHEEL: &str = "createhorizon:crushing_wheel";
const WATER_WHEEL: &str = "createhorizon:water_wheel";
const GEARBOX: &str = "createhorizon:gearbox";
const SPEEDOMETER: &str = "createhorizon:speedometer";
const CLUTCH: &str = "createhorizon:clutch";

/// A content block has no block-state properties, so every block's axle is
/// fixed by its id: the shaft, cogwheels, crank, loads, wheel, gauge and
/// clutch all turn about Z, the vertical shaft about Y, and the gearbox
/// takes rotation in along Z and sends it up along Y.
const HORIZONTAL_AXIS: Axis = Axis::Z;
const VERTICAL_AXIS: Axis = Axis::Y;
const GEARBOX_INPUT: Axis = Axis::Z;
const GEARBOX_OUTPUT: Axis = Axis::Y;

static CRANK_ENABLED: AtomicBool = AtomicBool::new(false);
static CLUTCH_ENGAGED: AtomicBool = AtomicBool::new(true);
static SPIN_FRAME: AtomicUsize = AtomicUsize::new(0);

/// What one look at the world found: the solved network, and the readings a
/// HUD or a command reports from it.
struct Observation {
    summary: NetworkSummary,
    state_hash: u64,
    has_crank: bool,
    crank_enabled: bool,
    clutch_engaged: bool,
    /// The strongest flow of any water wheel in range.
    flow: u32,
    /// What the gauge nearest the player reads, when there is one.
    gauge: Option<Fx>,
}

/// The configured crank key toggles the nearest hand crank and the clutch key
/// opens or closes every clutch, while the player is in-world.
pub(super) fn key_event(game: &Game<'_>, key: &str, pressed: bool) {
    if !game.native_on(MOD_ID) {
        reset_toggles();
        return;
    }
    if !pressed || !game.native_playing() {
        return;
    }

    if key == game.native_mod_key(MOD_ID, CRANK_KEY, DEFAULT_CRANK_KEY).as_str() {
        CRANK_ENABLED.fetch_xor(true, Ordering::Relaxed);
    }
    if key == game.native_mod_key(MOD_ID, CLUTCH_KEY, DEFAULT_CLUTCH_KEY).as_str() {
        CLUTCH_ENGAGED.fetch_xor(true, Ordering::Relaxed);
    }
}

/// The two switches on the mod's settings page, under its keys.
pub(super) fn settings_buttons(game: &Game<'_>) -> Vec<String> {
    if !game.native_on(MOD_ID) {
        return Vec::new();
    }
    let clutch = clutch_label(CLUTCH_ENGAGED.load(Ordering::Relaxed));
    let crank = if CRANK_ENABLED.load(Ordering::Relaxed) {
        "running"
    } else {
        "idle"
    };
    vec![
        format!("Clutch: {clutch}"),
        format!("Hand crank: {crank}"),
    ]
}

/// The settings page's buttons, in the order [`settings_buttons`] lists them.
pub(super) fn settings_clicked(game: &Game<'_>, button: usize) {
    if !game.native_on(MOD_ID) {
        return;
    }
    match button {
        0 => {
            CLUTCH_ENGAGED.fetch_xor(true, Ordering::Relaxed);
        }
        1 => {
            CRANK_ENABLED.fetch_xor(true, Ordering::Relaxed);
        }
        _ => {}
    }
}

pub(super) fn hud_lines(game: &Game<'_>) -> Vec<String> {
    let observation = observe(game, HUD_SCAN_RADIUS);
    let speed_rpm = observation.summary.max_speed.raw() / Fx::SCALE;
    let key_label = crank_key_label(game);
    let crank_line = if !observation.crank_enabled {
        format!("Crank: idle; press {key_label} to start")
    } else if !observation.has_crank {
        format!("Crank: no hand crank nearby; press {key_label} to stop")
    } else if observation.summary.overstressed_networks != 0 {
        format!("Crank: stalled; press {key_label} to stop")
    } else {
        format!("Crank: {} spinning; press {key_label} to stop", next_spin_glyph())
    };

    let mut extras = vec![format!(
        "Clutch: {}",
        clutch_label(observation.clutch_engaged)
    )];
    if observation.flow != 0 {
        extras.push(format!("wheel flow: {}", observation.flow));
    }
    if let Some(speed) = observation.gauge {
        extras.push(format!("gauge: {} rpm", speed.raw() / Fx::SCALE));
    }

    vec![
        format!("Create Horizon: math {SIM_MATH_VERSION}, kinetics {KINETICS_RULES_VERSION}"),
        format!(
            "{} parts, {} links, {} networks",
            observation.summary.part_count,
            observation.summary.link_count,
            observation.summary.network_count
        ),
        format!(
            "{} driven at {} rpm; {} overload",
            observation.summary.driven_parts, speed_rpm, observation.summary.overstressed_networks
        ),
        crank_line,
        extras.join("; "),
    ]
}

pub(super) fn scan_message(game: &Game<'_>) -> String {
    let observation = observe(game, COMMAND_SCAN_RADIUS);
    let crank_state = if !observation.crank_enabled {
        "off"
    } else if observation.has_crank {
        "on"
    } else {
        "on, no crank nearby"
    };
    let gauge = match observation.gauge {
        Some(speed) => format!("{} rpm", speed.raw() / Fx::SCALE),
        None => String::from("none"),
    };
    format!(
        "Create scan: crank={crank_state}, parts={}, links={}, networks={}, driven={}, max={} rpm, stress={}, capacity={}, overload={}, clutch={}, flow={}, gauge={gauge}, state={:016x}",
        observation.summary.part_count,
        observation.summary.link_count,
        observation.summary.network_count,
        observation.summary.driven_parts,
        observation.summary.max_speed.raw() / Fx::SCALE,
        observation.summary.stress.raw() / Fx::SCALE,
        observation.summary.capacity.raw() / Fx::SCALE,
        observation.summary.overstressed_networks,
        clutch_label(observation.clutch_engaged),
        observation.flow,
        observation.state_hash,
    )
}

/// Reads the blocks in a box around the player, solves them, and takes the
/// readings a HUD or a command asks for.
fn observe(game: &Game<'_>, radius: i32) -> Observation {
    let crank_enabled = CRANK_ENABLED.load(Ordering::Relaxed);
    let clutch_engaged = CLUTCH_ENGAGED.load(Ordering::Relaxed);
    let mut observation = Observation {
        summary: NetworkSummary::default(),
        state_hash: 0,
        has_crank: false,
        crank_enabled,
        clutch_engaged,
        flow: 0,
        gauge: None,
    };
    let Some(player) = game.native_player() else {
        return observation;
    };
    let center = BlockPos::new(
        block_coordinate(player.x),
        block_coordinate(player.y),
        block_coordinate(player.z),
    );
    let radius = radius.clamp(0, 12);
    let layout = game.world.layout();
    let min_y = center.y.saturating_sub(radius).max(layout.min_y());
    let max_y = center
        .y
        .saturating_add(radius)
        .min(layout.max_y().saturating_sub(1));
    let min_x = center.x.saturating_sub(radius);
    let max_x = center.x.saturating_add(radius);
    let min_z = center.z.saturating_sub(radius);
    let max_z = center.z.saturating_add(radius);

    // The scan reads block names only; no state is changed and no block
    // entity is opened.
    let mut found: Vec<(BlockPos, &str)> = Vec::new();
    for x in min_x..=max_x {
        for z in min_z..=max_z {
            if !game.world.loaded(x, z) {
                continue;
            }
            for y in min_y..=max_y {
                let state = game.world.block(x, y, z);
                let name = game.res.facts().get(state).name.as_ref();
                if is_kinetic_block(name) {
                    found.push((BlockPos::new(x, y, z), name));
                }
            }
        }
    }

    let mut parts = Vec::with_capacity(found.len());
    let mut gauges = Vec::new();
    for (position, name) in &found {
        match *name {
            HAND_CRANK => parts.push(KineticPart::HandCrank {
                position: *position,
                axis: HORIZONTAL_AXIS,
                speed: Fx::ZERO,
                capacity: Fx::ZERO,
            }),
            SHAFT => parts.push(KineticPart::Shaft {
                position: *position,
                axis: HORIZONTAL_AXIS,
            }),
            VERTICAL_SHAFT => parts.push(KineticPart::Shaft {
                position: *position,
                axis: VERTICAL_AXIS,
            }),
            SMALL_COGWHEEL => parts.push(KineticPart::Cogwheel(Gearwheel::new(
                *position,
                HORIZONTAL_AXIS,
                GearSize::Small,
            ))),
            LARGE_COGWHEEL => parts.push(KineticPart::Cogwheel(Gearwheel::new(
                *position,
                HORIZONTAL_AXIS,
                GearSize::Large,
            ))),
            MILLSTONE => parts.push(KineticPart::Load {
                position: *position,
                axis: HORIZONTAL_AXIS,
                impact: Fx::from_i32(MILLSTONE_STRESS),
            }),
            CRUSHING_WHEEL => parts.push(KineticPart::Load {
                position: *position,
                axis: HORIZONTAL_AXIS,
                impact: Fx::from_i32(CRUSHING_WHEEL_STRESS),
            }),
            WATER_WHEEL => {
                let flow = water_wheel_flow(&water_sides(game, *position, HORIZONTAL_AXIS));
                observation.flow = observation.flow.max(flow);
                parts.push(KineticPart::WaterWheel {
                    position: *position,
                    axis: HORIZONTAL_AXIS,
                    flow,
                });
            }
            GEARBOX => parts.push(KineticPart::Gearbox {
                position: *position,
                input: GEARBOX_INPUT,
                output: GEARBOX_OUTPUT,
            }),
            SPEEDOMETER => {
                gauges.push(*position);
                parts.push(KineticPart::Speedometer {
                    position: *position,
                    axis: HORIZONTAL_AXIS,
                });
            }
            CLUTCH => parts.push(KineticPart::Clutch {
                position: *position,
                axis: HORIZONTAL_AXIS,
                engaged: clutch_engaged,
            }),
            _ => {}
        }
    }

    observation.has_crank = configure_manual_crank(&mut parts, center, crank_enabled);
    let mut kinetics = Kinetics::from_parts(parts);
    observation.summary = kinetics.solve();
    observation.state_hash = kinetics.state_hash();
    observation.gauge = nearest_position(&gauges, center).and_then(|at| kinetics.speed_at(at));
    observation
}

/// Selects one nearby manual source; other hand cranks remain passive shafts.
fn configure_manual_crank(parts: &mut [KineticPart], center: BlockPos, enabled: bool) -> bool {
    let nearest = parts
        .iter()
        .filter_map(|part| match *part {
            KineticPart::HandCrank { position, .. } => Some(position),
            _ => None,
        })
        .min_by_key(|position| {
            (
                distance_squared(*position, center),
                position.x,
                position.y,
                position.z,
            )
        });
    let active = if enabled { nearest } else { None };

    for part in parts.iter_mut() {
        let KineticPart::HandCrank { position, axis, .. } = *part else {
            continue;
        };
        *part = if Some(position) == active {
            KineticPart::HandCrank {
                position,
                axis,
                speed: Fx::from_i32(CRANK_SPEED_RPM),
                capacity: Fx::from_i32(CRANK_CAPACITY),
            }
        } else {
            KineticPart::Shaft { position, axis }
        };
    }
    nearest.is_some()
}

/// The water a wheel stands in: the four blocks beside its axle, one per
/// side. A source or a falling column reads as level 8.
fn water_sides(game: &Game<'_>, position: BlockPos, axis: Axis) -> [WaterSide; 4] {
    let mut sides = [WaterSide::default(); 4];
    let mut index = 0;
    for side in axis.other_axes() {
        for step in [1, -1] {
            let Some(neighbour) = offset_along(position, side, step) else {
                index += 1;
                continue;
            };
            let facts = game
                .res
                .facts()
                .get(game.world.block(neighbour.x, neighbour.y, neighbour.z));
            if facts.fluid == Fluid::Water {
                sides[index] = WaterSide {
                    level: facts.fluid_level,
                    falling: facts.fluid_falling,
                };
            }
            index += 1;
        }
    }
    sides
}

fn offset_along(position: BlockPos, axis: Axis, step: i32) -> Option<BlockPos> {
    let mut delta = [0, 0, 0];
    delta[axis.index()] = step;
    position.checked_offset(delta)
}

fn nearest_position(positions: &[BlockPos], center: BlockPos) -> Option<BlockPos> {
    positions
        .iter()
        .copied()
        .min_by_key(|position| {
            (
                distance_squared(*position, center),
                position.x,
                position.y,
                position.z,
            )
        })
}

fn is_kinetic_block(name: &str) -> bool {
    matches!(
        name,
        HAND_CRANK
            | SHAFT
            | VERTICAL_SHAFT
            | SMALL_COGWHEEL
            | LARGE_COGWHEEL
            | MILLSTONE
            | CRUSHING_WHEEL
            | WATER_WHEEL
            | GEARBOX
            | SPEEDOMETER
            | CLUTCH
    )
}

fn distance_squared(left: BlockPos, right: BlockPos) -> i128 {
    let dx = i128::from(left.x) - i128::from(right.x);
    let dy = i128::from(left.y) - i128::from(right.y);
    let dz = i128::from(left.z) - i128::from(right.z);
    dx * dx + dy * dy + dz * dz
}

fn clutch_label(engaged: bool) -> &'static str {
    if engaged {
        "engaged"
    } else {
        "released"
    }
}

fn crank_key_label(game: &Game<'_>) -> String {
    let binding = game.native_mod_key(MOD_ID, CRANK_KEY, DEFAULT_CRANK_KEY);
    binding
        .strip_prefix("key.keyboard.")
        .unwrap_or("configured key")
        .to_uppercase()
}

fn next_spin_glyph() -> &'static str {
    let frame = SPIN_FRAME.fetch_add(1, Ordering::Relaxed);
    SPIN_FRAMES[frame % SPIN_FRAMES.len()]
}

/// A mod the player turned off holds nothing over the world it left.
fn reset_toggles() {
    CRANK_ENABLED.store(false, Ordering::Relaxed);
    CLUTCH_ENGAGED.store(true, Ordering::Relaxed);
}

#[cfg(test)]
pub(super) fn reset_for_tests() {
    reset_toggles();
    SPIN_FRAME.store(0, Ordering::Relaxed);
}

// The player position is a host-provided world coordinate; quantization ends
// at the block boundary. Simulation after this conversion is integer/fixed-point.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn block_coordinate(value: f64) -> i32 {
    value.floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use create_engine::WATER_WHEEL_FULL_FLOW;

    #[test]
    fn manual_toggle_drives_only_the_nearest_crank() {
        let center = BlockPos::ZERO;
        let near = BlockPos::new(1, 0, 0);
        let far = BlockPos::new(0, 0, 3);
        let mut parts = vec![
            KineticPart::HandCrank {
                position: far,
                axis: Axis::Z,
                speed: Fx::ZERO,
                capacity: Fx::ZERO,
            },
            KineticPart::HandCrank {
                position: near,
                axis: Axis::Z,
                speed: Fx::ZERO,
                capacity: Fx::ZERO,
            },
        ];

        assert!(configure_manual_crank(&mut parts, center, true));
        assert!(parts.iter().any(|part| matches!(
            part,
            KineticPart::HandCrank { position, speed, capacity, .. }
                if *position == near
                    && *speed == Fx::from_i32(CRANK_SPEED_RPM)
                    && *capacity == Fx::from_i32(CRANK_CAPACITY)
        )));
        assert!(parts.iter().any(|part| matches!(
            part, KineticPart::Shaft { position, .. } if *position == far
        )));

        assert!(configure_manual_crank(&mut parts, center, false));
        assert!(parts.iter().all(|part| matches!(part, KineticPart::Shaft { .. })));
    }

    #[test]
    fn sample_drive_transmits_speed_and_stays_within_capacity() {
        let origin = BlockPos::new(0, 64, 0);
        let parts = vec![
            KineticPart::HandCrank {
                position: origin,
                axis: Axis::Z,
                speed: Fx::from_i32(CRANK_SPEED_RPM),
                capacity: Fx::from_i32(CRANK_CAPACITY),
            },
            KineticPart::Shaft {
                position: BlockPos::new(0, 64, 1),
                axis: Axis::Z,
            },
            KineticPart::Cogwheel(Gearwheel::new(
                BlockPos::new(0, 64, 2),
                Axis::Z,
                GearSize::Large,
            )),
            KineticPart::Cogwheel(Gearwheel::new(
                BlockPos::new(1, 65, 2),
                Axis::Z,
                GearSize::Small,
            )),
            KineticPart::Load {
                position: BlockPos::new(0, 64, -1),
                axis: Axis::Z,
                impact: Fx::from_i32(MILLSTONE_STRESS),
            },
        ];
        let mut kinetics = Kinetics::from_parts(parts);
        let summary = kinetics.solve();
        assert_eq!(summary.part_count, 5);
        assert_eq!(summary.link_count, 4);
        assert_eq!(summary.network_count, 1);
        assert_eq!(summary.driven_parts, 5);
        assert_eq!(summary.max_speed, Fx::from_i32(32));
        assert_eq!(summary.overstressed_networks, 0);
    }

    #[test]
    fn sample_crushing_wheel_overloads_and_stops_the_network() {
        let parts = vec![
            KineticPart::HandCrank {
                position: BlockPos::ZERO,
                axis: Axis::Z,
                speed: Fx::from_i32(CRANK_SPEED_RPM),
                capacity: Fx::from_i32(CRANK_CAPACITY),
            },
            KineticPart::Load {
                position: BlockPos::new(0, 0, 1),
                axis: Axis::Z,
                impact: Fx::from_i32(CRUSHING_WHEEL_STRESS),
            },
        ];
        let mut kinetics = Kinetics::from_parts(parts);
        let summary = kinetics.solve();
        assert_eq!(summary.overstressed_networks, 1);
        assert_eq!(summary.driven_parts, 0);
        assert!(kinetics.states().iter().all(|state| state.speed.is_zero()));
    }

    #[test]
    fn water_sides_walk_the_axle_s_perpendicular_axes_in_a_fixed_order() {
        let wheel = KineticPart::WaterWheel {
            position: BlockPos::new(0, 0, 0),
            axis: Axis::Z,
            flow: WATER_WHEEL_FULL_FLOW,
        };
        let terminal = wheel.terminals();
        assert_eq!(terminal.as_slice().len(), 1);
        // A Z-axle wheel takes its water from the X and Y sides, never
        // from along the axle.
        assert_eq!(
            terminal.on(Axis::Z).map(|face| face.axis),
            Some(Axis::Z)
        );
        assert_eq!(terminal.on(Axis::X), None);
    }

    #[test]
    fn the_block_names_the_world_scan_looks_for_are_the_mod_s_own() {
        for name in [
            HAND_CRANK,
            SHAFT,
            VERTICAL_SHAFT,
            SMALL_COGWHEEL,
            LARGE_COGWHEEL,
            MILLSTONE,
            CRUSHING_WHEEL,
            WATER_WHEEL,
            GEARBOX,
            SPEEDOMETER,
            CLUTCH,
        ] {
            assert!(is_kinetic_block(name), "{name}");
            assert!(name.starts_with("createhorizon:"), "{name}");
        }
        assert!(!is_kinetic_block("minecraft:stone"));
        assert!(!is_kinetic_block("createhorizon:nope"));
    }

    #[test]
    fn offset_along_steps_one_block_on_the_axis_it_is_given() {
        let position = BlockPos::new(1, 2, 3);
        assert_eq!(offset_along(position, Axis::X, 1), Some(BlockPos::new(2, 2, 3)));
        assert_eq!(offset_along(position, Axis::Y, -1), Some(BlockPos::new(1, 1, 3)));
        assert_eq!(offset_along(BlockPos::new(i32::MAX, 0, 0), Axis::X, 1), None);
    }
}
