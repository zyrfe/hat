//! HAT front end: Bevy rendering, input, UI and audio over the `hat_sim` world.

mod audio;
mod cab;
mod debug;
mod camera;
mod input;
mod light;
mod render;
mod sim;
mod terrain;
mod ui;

use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, WindowMode};
use bevy_egui::EguiPlugin;

fn main() {
    App::new()
        .insert_resource(ClearColor(light::SKY))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "HAT: Huge-Ass Trains".into(),
                mode: WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(EguiPlugin::default())
        .insert_resource(Time::<Fixed>::from_hz(120.0))
        .add_plugins((sim::SimPlugin, camera::CameraPlugin, render::RenderPlugin, light::LightPlugin, input::InputPlugin, ui::UiPlugin, audio::AudioPlugin, cab::CabPlugin, debug::DebugPlugin))
        .run();
}
