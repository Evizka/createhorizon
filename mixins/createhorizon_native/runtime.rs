// This module is injected into Horizon's game crate by horizon-mixins.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use create_engine::{
    Axis, BlockPos, Fx, GearSize, Gearwheel, KineticPart, Kinetics, NetworkSummary,
    SIM_MATH_VERSION, KINETICS_RULES_VERSION,
};
use super::Game;

const MOD_ID: &str = "createhorizon_native";
const CRANK_KEY: &str = "crank";
const DEFAULT_CRANK_KEY: &str = "key.keyboard.c";
const CRANK_SPEED_RPM: i32 = 16;
const CRANK_CAPACITY: i32 = 16;
const MILLSTONE_STRESS: i32 = 12;
const CRUSHING_WHEEL_STRESS: i32 = 24;
const HUD_SCAN_RADIUS: i32 = 3;
const COMMAND_SCAN_RADIUS: i32 = 8;
const SPIN_FRAMES: [&str; 4] = ["|", "/", "-", "\\"];

static CRANK_ENABLED: AtomicBool = AtomicBool::new(false);
static SPIN_FRAME: AtomicUsize = AtomicUsize::new(0);

/// The configured key toggles the nearest crank while the player is in-world.
pub(super) fn key_event(game: &Game<'_>, key: &str, pressed: bool) {
    if !game.native_on(MOD_ID) {
        CRANK_ENABLED.store(false, Ordering::Relaxed);
        return;
    }
    if !pressed || !game.native_playing() {
        return;
    }

    let binding = game.native_mod_key(MOD_ID, CRANK_KEY, DEFAULT_CRANK_KEY);
    if key == binding.as_str() {
        CRANK_ENABLED.fetch_xor(true, Ordering::Relaxed);
    }
}

pub(super) fn hud_lines(game: &Game<'_>) -> [String; 4] {
    let (summary, _, has_crank, crank_enabled) = observe(game, HUD_SCAN_RADIUS);
    let speed_rpm = summary.max_speed.raw() / Fx::SCALE;
    let key_label = crank_key_label(game);
    let crank_line = if !crank_enabled {
        format!("Crank: idle; press {key_label} to start")
    } else if !has_crank {
        format!("Crank: no hand crank nearby; press {key_label} to stop")
    } else if summary.overstressed_networks != 0 {
        format!("Crank: stalled; press {key_label} to stop")
    } else {
        format!("Crank: {} spinning; press {key_label} to stop", next_spin_glyph())
    };

    [
        format!("Create Horizon: math {SIM_MATH_VERSION}, kinetics {KINETICS_RULES_VERSION}"),
        format!(
            "{} parts, {} links, {} networks",
            summary.part_count, summary.link_count, summary.network_count
        ),
        format!(
            "{} driven at {} rpm; {} overload",
            summary.driven_parts, speed_rpm, summary.overstressed_networks
        ),
        crank_line,
    ]
}

pub(super) fn scan_message(game: &Game<'_>) -> String {
    let (summary, state_hash, has_crank, crank_enabled) = observe(game, COMMAND_SCAN_RADIUS);
    let crank_state = if !crank_enabled {
        "off"
    } else if has_crank {
        "on"
    } else {
        "on, no crank nearby"
    };
    format!(
        "Create scan: crank={crank_state}, parts={}, links={}, networks={}, driven={}, max={} rpm, stress={}, capacity={}, overload={}, state={state_hash:016x}",
        summary.part_count,
        summary.link_count,
        summary.network_count,
        summary.driven_parts,
        summary.max_speed.raw() / Fx::SCALE,
        summary.stress.raw() / Fx::SCALE,
        summary.capacity.raw() / Fx::SCALE,
        summary.overstressed_networks,
    )
}

fn observe(game: &Game<'_>, radius: i32) -> (NetworkSummary, u64, bool, bool) {
    let crank_enabled = CRANK_ENABLED.load(Ordering::Relaxed);
    let Some(player) = game.native_player() else {
        return (NetworkSummary::default(), 0, false, crank_enabled);
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
    let mut parts = Vec::new();

    for x in min_x..=max_x {
        for z in min_z..=max_z {
            if !game.world.loaded(x, z) {
                continue;
            }
            for y in min_y..=max_y {
                let position = BlockPos::new(x, y, z);
                let state = game.world.block(x, y, z);
                let name = game.res.facts().get(state).name.as_ref();
                let part = match name {
                    "createhorizon:hand_crank" => Some(KineticPart::HandCrank {
                        position,
                        axis: Axis::Z,
                        speed: Fx::ZERO,
                        capacity: Fx::ZERO,
                    }),
                    "createhorizon:shaft" => Some(KineticPart::Shaft {
                        position,
                        axis: Axis::Z,
                    }),
                    "createhorizon:small_cogwheel" => Some(KineticPart::Cogwheel(
                        Gearwheel::new(position, Axis::Z, GearSize::Small),
                    )),
                    "createhorizon:large_cogwheel" => Some(KineticPart::Cogwheel(
                        Gearwheel::new(position, Axis::Z, GearSize::Large),
                    )),
                    "createhorizon:millstone" => Some(KineticPart::Load {
                        position,
                        axis: Axis::Z,
                        impact: Fx::from_i32(MILLSTONE_STRESS),
                    }),
                    "createhorizon:crushing_wheel" => Some(KineticPart::Load {
                        position,
                        axis: Axis::Z,
                        impact: Fx::from_i32(CRUSHING_WHEEL_STRESS),
                    }),
                    _ => None,
                };
                if let Some(part) = part {
                    parts.push(part);
                }
            }
        }
    }

    let has_crank = configure_manual_crank(&mut parts, center, crank_enabled);
    let mut kinetics = Kinetics::from_parts(parts);
    let summary = kinetics.solve();
    (summary, kinetics.state_hash(), has_crank, crank_enabled)
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

fn distance_squared(left: BlockPos, right: BlockPos) -> i128 {
    let dx = i128::from(left.x) - i128::from(right.x);
    let dy = i128::from(left.y) - i128::from(right.y);
    let dz = i128::from(left.z) - i128::from(right.z);
    dx * dx + dy * dy + dz * dz
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

// The player position is a host-provided world coordinate; quantization ends
// at the block boundary. Simulation after this conversion is integer/fixed-point.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn block_coordinate(value: f64) -> i32 {
    value.floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

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
            part,
            KineticPart::Shaft { position, .. } if *position == far
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
}
