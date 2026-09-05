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
    }
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
