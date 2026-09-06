//! Developer hooks. `HAT_SCREENSHOT=/path/shot.png` saves a frame a few seconds after
//! start and exits; independent of whether a display is awake, so it works from scripts.

use bevy::prelude::*;
use bevy::render::view::window::screenshot::{save_to_disk, Screenshot};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        if let Ok(path) = std::env::var("HAT_SCREENSHOT") {
            let delay: f32 = std::env::var("HAT_SCREENSHOT_DELAY").ok().and_then(|s| s.parse().ok()).unwrap_or(4.0);
            app.insert_resource(ShotPlan { path, delay, taken: false }).add_systems(Update, take_shot);
        }
        if std::env::var("HAT_FPS_LOG").is_ok() {
            app.insert_resource(FpsLog { next: 1.0 }).add_systems(Update, log_fps);
        }
    }
}

#[derive(Resource)]
struct FpsLog {
    next: f32,
}

/// `HAT_FPS_LOG=1`: one line a second on stderr with frame rate, sim time and clock.
fn log_fps(time: Res<Time>, mut log: ResMut<FpsLog>, diag: Res<bevy::diagnostic::DiagnosticsStore>, sim: Res<crate::sim::Sim>) {
    if time.elapsed_secs() < log.next {
        return;
    }
    log.next += 1.0;
    let fps = diag.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let ms = diag.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FRAME_TIME).and_then(|d| d.smoothed()).unwrap_or(0.0);
    eprintln!("[fps] wall {:>5.1}s  {:>5.1} fps  {:>6.2} ms  sim t {:>7.1}s  x{}  trains {}", time.elapsed_secs(), fps, ms, sim.world.t, sim.time_scale, sim.world.trains.len());
}

#[derive(Resource)]
struct ShotPlan {
    path: String,
    delay: f32,
    taken: bool,
}

fn take_shot(mut commands: Commands, time: Res<Time>, mut plan: ResMut<ShotPlan>, mut exit: MessageWriter<AppExit>, cab: Option<Res<crate::cab::CabView>>) {
    if !plan.taken && time.elapsed_secs() >= plan.delay {
        plan.taken = true;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(plan.path.clone()));
        // The cab view renders to a texture; it does not depend on the window presenting.
        if let Some(cab) = cab {
            let cab_path = format!("{}.cab.png", plan.path.trim_end_matches(".png"));
            commands.spawn(Screenshot::image(cab.image())).observe(save_to_disk(cab_path));
        }
    }
    if plan.taken && time.elapsed_secs() >= plan.delay + 2.0 {
        exit.write(AppExit::Success);
    }
}
