//! Sun, sky and air. A low raking sun with cascaded shadows, a gradient-cube hemisphere for
//! ambient, screen-space ambient occlusion to seat things on the ground, and distance fog
//! that scales with the zoom so far country recedes at every height.

use bevy::anti_alias::smaa::Smaa;
use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap};
use bevy::pbr::{DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension};

use crate::cab::CabCamera;
use crate::camera::{MainCamera, Rig};

pub struct LightPlugin;

impl Plugin for LightPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(PostStartup, dress_cameras).add_systems(Update, fog_follows_zoom);
    }
}

/// Haze and clear colour. Pale so the horizon reads as air, not night.
pub const SKY: Color = Color::srgb(0.60, 0.70, 0.82);
/// Fog visibility as a multiple of camera distance. Keeps the far edge of any view a
/// little hazier than the near edge without ever washing out the yard.
const VISIBILITY_PER_DISTANCE: f32 = 90.0;

fn setup(mut commands: Commands) {
    commands.insert_resource(GlobalAmbientLight { color: Color::srgb(0.78, 0.84, 1.0), brightness: 60.0, ..default() });
    commands.insert_resource(DirectionalLightShadowMap { size: 4096 });
    // From the north-west, 38 degrees up. Shadows fall toward the viewer at the default
    // yaw and rake across the east-west yards, so embankment faces differ from their tops.
    let (az, el) = (45f32.to_radians(), 38f32.to_radians());
    let toward_scene = Vec3::new(el.cos() * az.sin(), -el.sin(), el.cos() * az.cos());
    commands.spawn((
        DirectionalLight { illuminance: 13_000.0, color: Color::srgb(1.0, 0.96, 0.90), shadow_maps_enabled: true, ..default() },
        CascadeShadowConfigBuilder { num_cascades: 4, minimum_distance: 1.0, maximum_distance: 1600.0, first_cascade_far_bound: 70.0, overlap_proportion: 0.15 }.build(),
        Transform::default().looking_to(toward_scene, Vec3::Y),
    ));
}

fn fog(visibility: f32) -> DistanceFog {
    DistanceFog {
        color: SKY,
        directional_light_color: Color::srgba(1.0, 0.95, 0.85, 0.25),
        directional_light_exponent: 30.0,
        falloff: FogFalloff::from_visibility_colors(visibility, Color::srgb(0.35, 0.5, 0.66), Color::srgb(0.8, 0.844, 1.0)),
    }
}

/// The cameras exist after the camera and cab plugins' startup; give them their air.
fn dress_cameras(mut commands: Commands, mut images: ResMut<Assets<Image>>, main: Single<Entity, With<MainCamera>>, cab: Single<Entity, With<CabCamera>>) {
    let sky = images.add(sky_cube());
    let hemisphere = || EnvironmentMapLight { diffuse_map: sky.clone(), specular_map: sky.clone(), intensity: 260.0, ..default() };
    commands.entity(*main).insert((Msaa::Off, ScreenSpaceAmbientOcclusion::default(), Smaa::default(), fog(24_000.0), hemisphere()));
    commands.entity(*cab).insert((fog(12_000.0), hemisphere()));
}

fn fog_follows_zoom(rig: Res<Rig>, mut q: Query<&mut DistanceFog, With<MainCamera>>) {
    for mut f in &mut q {
        *f = fog(rig.distance * VISIBILITY_PER_DISTANCE);
    }
}

/// A tiny cubemap: blue overhead, pale at the horizon, warm ground bounce below. Used as
/// both the diffuse and the specular environment, which is all a smooth gradient needs.
fn sky_cube() -> Image {
    const N: u32 = 16;
    let zenith = Vec3::new(0.40, 0.58, 0.95);
    let horizon = Vec3::new(0.86, 0.90, 0.96);
    let ground = Vec3::new(0.38, 0.36, 0.30);
    let mut data = Vec::with_capacity((N * N * 6 * 4) as usize);
    for face in 0..6 {
        for row in 0..N {
            for col in 0..N {
                let u = (col as f32 + 0.5) / N as f32 * 2.0 - 1.0;
                let v = (row as f32 + 0.5) / N as f32 * 2.0 - 1.0;
                // wgpu face order: +X, -X, +Y, -Y, +Z, -Z.
                let dir = match face {
                    0 => Vec3::new(1.0, -v, -u),
                    1 => Vec3::new(-1.0, -v, u),
                    2 => Vec3::new(u, 1.0, v),
                    3 => Vec3::new(u, -1.0, -v),
                    4 => Vec3::new(u, -v, 1.0),
                    _ => Vec3::new(-u, -v, -1.0),
                }
                .normalize();
                let c = if dir.y >= 0.0 { horizon.lerp(zenith, dir.y.powf(0.6)) } else { horizon.lerp(ground, (-dir.y).powf(0.5)) };
                for ch in [c.x, c.y, c.z, 1.0] {
                    data.push((ch.clamp(0.0, 1.0) * 255.0).round() as u8);
                }
            }
        }
    }
    let mut image = Image::new(Extent3d { width: N, height: N, depth_or_array_layers: 6 }, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
    image.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..default() });
    image
}
