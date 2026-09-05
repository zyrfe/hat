//! Synthesized placeholder sounds driven by sim events, plus an engine drone.

use std::sync::Arc;

use bevy::audio::{AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use hat_sim::*;

use crate::camera::{sim_to_world, Rig};
use crate::sim::Sim;

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, (play_events, engine));
    }
}

#[derive(Resource)]
pub struct Sounds {
    bang: Handle<AudioSource>,
    clack: Handle<AudioSource>,
    hiss: Handle<AudioSource>,
    emergency: Handle<AudioSource>,
    crash: Handle<AudioSource>,
    switch: Handle<AudioSource>,
}

#[derive(Component)]
struct Engine;

#[derive(Resource, Default)]
struct Jitter(u32);

const RATE: u32 = 44_100;

struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

fn env(t: f32, tau: f32) -> f32 {
    (-t / tau).exp()
}

fn sine(f: f32, t: f32) -> f32 {
    (std::f32::consts::TAU * f * t).sin()
}

fn render(seconds: f32, mut f: impl FnMut(f32, usize) -> f32) -> Vec<f32> {
    let n = (seconds * RATE as f32) as usize;
    (0..n).map(|i| f(i as f32 / RATE as f32, i).clamp(-1.0, 1.0)).collect()
}

fn bang() -> Vec<f32> {
    let mut noise = Noise(1);
    let mut lp = 0.0f32;
    render(0.45, move |t, _| {
        let raw = noise.next();
        lp += 0.22 * (raw - lp);
        lp * env(t, 0.035) * 1.3 + sine(55.0, t) * env(t, 0.18) * 0.8 + sine(110.0, t) * env(t, 0.06) * 0.35 + raw * env(t, 0.006) * 0.7
    })
}

fn clack() -> Vec<f32> {
    let mut noise = Noise(2);
    render(0.12, move |t, _| noise.next() * env(t, 0.012) * 0.9 + sine(900.0, t) * env(t, 0.02) * 0.3)
}

fn hiss() -> Vec<f32> {
    let mut noise = Noise(3);
    let mut lp = 0.0f32;
    render(1.4, move |t, _| {
        lp += 0.5 * (noise.next() - lp);
        let k = (1.0 - t / 1.4).max(0.0);
        lp * k * k * 0.7
    })
}

fn emergency() -> Vec<f32> {
    let mut noise = Noise(4);
    let mut lp = 0.0f32;
    render(3.2, move |t, _| {
        lp += 0.7 * (noise.next() - lp);
        let k = (1.0 - t / 3.2).max(0.0).powf(1.4);
        lp * k * 0.95 + sine(40.0, t) * env(t, 0.5) * 0.3
    })
}

fn crash() -> Vec<f32> {
    let mut noise = Noise(5);
    let mut lp = 0.0f32;
    render(1.8, move |t, _| {
        let raw = noise.next();
        lp += 0.3 * (raw - lp);
        lp * env(t, 0.4) * 1.0 + sine(35.0, t) * env(t, 0.8) * 0.6 + sine(320.0, t) * env(t, 0.25) * 0.25 + sine(770.0, t) * env(t, 0.15) * 0.2 + raw * env(t, 0.01) * 0.8
    })
}

fn switch() -> Vec<f32> {
    let mut noise = Noise(6);
    render(0.3, move |t, _| {
        let t2 = (t - 0.13).max(0.0);
        noise.next() * (env(t, 0.004) * 0.8 + if t >= 0.13 { env(t2, 0.005) * 0.6 } else { 0.0 })
    })
}

fn engine_loop() -> Vec<f32> {
    let mut noise = Noise(7);
    let mut lp = 0.0f32;
    let mut v = render(1.0, move |t, _| {
        lp += 0.15 * (noise.next() - lp);
        let ph = (t * 15.0).fract();
        let pulse = if ph < 0.12 { 1.0 - ph / 0.12 } else { 0.0 };
        sine(45.0, t) * 0.45 + sine(90.0, t) * 0.25 + sine(135.0, t) * 0.1 + pulse * lp * 0.6
    });
    let n = v.len();
    let fade = 600;
    for i in 0..fade {
        let k = i as f32 / fade as f32;
        let tail = v[n - fade + i];
        v[n - fade + i] = tail * (1.0 - k) + v[i] * k;
    }
    v
}

fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&((s * 32_000.0) as i16).to_le_bytes());
    }
    out
}

fn setup(mut commands: Commands, mut assets: ResMut<Assets<AudioSource>>) {
    let mut mk = |v: Vec<f32>| assets.add(AudioSource { bytes: Arc::from(wav(&v)) });
    let sounds = Sounds { bang: mk(bang()), clack: mk(clack()), hiss: mk(hiss()), emergency: mk(emergency()), crash: mk(crash()), switch: mk(switch()) };
    let engine = mk(engine_loop());
    commands.spawn((AudioPlayer(engine), PlaybackSettings { volume: Volume::Linear(0.2), ..PlaybackSettings::LOOP }, Engine));
    commands.insert_resource(sounds);
    commands.insert_resource(Jitter(0));
}

fn play_events(mut commands: Commands, mut sim: ResMut<Sim>, sounds: Option<Res<Sounds>>, rig: Res<Rig>, mut jitter: ResMut<Jitter>) {
    let Some(sounds) = sounds else { return };
    let events = std::mem::take(&mut sim.pending_events);
    let mut budget = 8;
    for e in events {
        let (handle, base, speed, pos) = match e {
            SimEvent::CouplerImpact { pos, rel_speed, mass } => (&sounds.bang, (rel_speed / 2.0).clamp(0.12, 1.0) * (mass / 200_000.0).clamp(0.5, 1.5), 0.85, pos),
            SimEvent::Coupled { pos, rel_speed, .. } => (&sounds.bang, (0.4 + rel_speed / 2.0).min(1.2), 0.75, pos),
            SimEvent::Bumped { pos, rel_speed } => {
                if rel_speed < 0.08 {
                    continue;
                }
                (&sounds.bang, (rel_speed / 2.0).clamp(0.1, 1.0), 0.8, pos)
            }
            SimEvent::Uncoupled { pos, .. } => (&sounds.clack, 0.6, 1.0, pos),
            SimEvent::KnuckleBreak { pos, .. } => (&sounds.crash, 1.0, 1.1, pos),
            SimEvent::PipeVent { pos, .. } => (&sounds.hiss, 0.8, 1.0, pos),
            SimEvent::Emergency { pos, .. } => (&sounds.emergency, 0.9, 1.0, pos),
            SimEvent::Derail { pos, .. } => (&sounds.crash, 1.0, 0.9, pos),
            SimEvent::BumperHit { pos, speed, .. } => (&sounds.bang, (speed / 2.0).clamp(0.3, 1.2), 0.7, pos),
            SimEvent::SwitchThrown { pos, .. } => (&sounds.switch, 0.6, 1.0, pos),
            SimEvent::HandBrake { pos, .. } => (&sounds.clack, 0.35, 1.6, pos),
            SimEvent::Bled { pos, .. } => (&sounds.hiss, 0.45, 1.3, pos),
            SimEvent::HosesConnected { pos, .. } => (&sounds.hiss, 0.35, 1.1, pos),
            SimEvent::AirBottled { pos, .. } => (&sounds.clack, 0.35, 1.2, pos),
            SimEvent::Loaded { pos, .. } => (&sounds.hiss, 0.3, 0.7, pos),
            SimEvent::Unloaded { pos, .. } => (&sounds.hiss, 0.3, 0.6, pos),
        };
        let d = sim_to_world(pos).distance(rig.focus);
        let reach = rig.distance * 3.0 + 250.0;
        let attenuation = (1.0 - d / reach).clamp(0.0, 1.0);
        let zoom = (350.0 / (rig.distance + 350.0)).clamp(0.25, 1.0);
        let gain = base as f32 * attenuation * zoom;
        if gain < 0.02 {
            continue;
        }
        if budget == 0 {
            break;
        }
        budget -= 1;
        jitter.0 = jitter.0.wrapping_mul(1664525).wrapping_add(1013904223);
        let j = 0.92 + (jitter.0 >> 24) as f32 / 255.0 * 0.16;
        commands.spawn((AudioPlayer(handle.clone()), PlaybackSettings { volume: Volume::Linear(gain), speed: speed * j, ..PlaybackSettings::DESPAWN }));
    }
}

fn engine(sim: Res<Sim>, rig: Res<Rig>, mut q: Query<&mut AudioSink, With<Engine>>) {
    let notch = sim.controls.throttle as f32;
    let v = sim.loco_speed().abs() as f32;
    let zoom = (350.0 / (rig.distance + 350.0)).clamp(0.15, 1.0);
    for mut sink in &mut q {
        sink.set_speed(0.85 + notch * 0.09 + v * 0.008);
        sink.set_volume(Volume::Linear((0.10 + notch * 0.035) * zoom));
    }
}
