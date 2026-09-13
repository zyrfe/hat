//! Developer hooks. `HAT_SCREENSHOT=/path/shot.png` saves a frame a few seconds after
//! start and exits. In that mode the main camera renders to an offscreen image, so the
//! frame is valid whether or not a display is awake, and the UI is left out of it.

use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::window::screenshot::{save_to_disk, Screenshot};

use crate::camera::MainCamera;

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        if let Ok(path) = std::env::var("HAT_SCREENSHOT") {
            let delay: f32 = std::env::var("HAT_SCREENSHOT_DELAY").ok().and_then(|s| s.parse().ok()).unwrap_or(4.0);
            app.insert_resource(ShotPlan { path, delay, taken: false, target: None }).add_systems(PostStartup, offscreen_target).add_systems(Update, take_shot);
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
    /// The main camera's offscreen target while shooting.
    target: Option<Handle<Image>>,
}

/// `HAT_SCREENSHOT_SIZE=WxH` sets the offscreen frame; default 2560x1440.
fn offscreen_target(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut plan: ResMut<ShotPlan>, cam: Single<Entity, With<MainCamera>>) {
    let (w, h) = std::env::var("HAT_SCREENSHOT_SIZE").ok().and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))).unwrap_or((2560u32, 1440u32));
    let image = images.add(Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None));
    commands.entity(*cam).insert(RenderTarget::from(image.clone()));
    plan.target = Some(image);
}

fn take_shot(mut commands: Commands, time: Res<Time>, mut plan: ResMut<ShotPlan>, mut exit: MessageWriter<AppExit>, cab: Option<Res<crate::cab::CabView>>) {
    if !plan.taken && time.elapsed_secs() >= plan.delay {
        plan.taken = true;
        let shot = match &plan.target {
            Some(img) => Screenshot::image(img.clone()),
            None => Screenshot::primary_window(),
        };
        commands.spawn(shot).observe(save_to_disk(plan.path.clone()));
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
