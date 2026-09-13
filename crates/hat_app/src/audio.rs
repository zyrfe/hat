//! Synthesized placeholder sounds driven by sim events, plus a diesel that idles and
//! loads: two exactly periodic firing-pulse loops blended by a smoothed rpm.

use std::f32::consts::TAU;
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

/// One of the two diesel loops. Idle is soft and low; load barks and whines.
#[derive(Component)]
struct Engine {
    load: bool,
}

/// Engine speed as a fraction of the notch range, eased toward the notch so pitch and
/// tone slew like a prime mover instead of stepping.
#[derive(Resource, Default)]
struct Rpm(f32);

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
    (TAU * f * t).sin()
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

/// Firings per second in the rendered loop. The sink's speed pitches it up to notch eight.
const ENGINE_F0: f32 = 48.0;
/// Firings in the loop: one second exactly, so every component is periodic in the buffer.
const ENGINE_CYCLES: usize = 48;
/// Firings per crankshaft revolution: a two-stroke V16 fires every cylinder every turn.
const FIRINGS_PER_REV: usize = 16;

/// A diesel loop with no seam: each firing is a short resonant bark plus a noise puff,
/// added into the buffer modulo its length so tails wrap around; the low-pass runs over
/// the buffer twice so its state matches at the join; the peak is normalised so nothing clips.
fn engine_loop(load: bool) -> Vec<f32> {
    let len = (RATE as usize * ENGINE_CYCLES) / ENGINE_F0 as usize;
    let mut v = vec![0.0f32; len];
    let mut noise = Noise(if load { 8 } else { 7 });
    let (bark_hz, bark_tau, puff_tau, puff_lp, puff_gain) = if load { (210.0, 0.009, 0.010, 0.35, 0.9) } else { (150.0, 0.006, 0.007, 0.15, 0.55) };
    let pulse_len = (RATE as f32 * 0.06) as usize;
    for k in 0..ENGINE_CYCLES {
        let start = k * len / ENGINE_CYCLES;
        // Cylinders differ a little, and the crank turn gives a slow beat under the firing rate.
        let unevenness = 1.0 + 0.18 * noise.next();
        let crank = 1.0 + 0.12 * (TAU * (k % FIRINGS_PER_REV) as f32 / FIRINGS_PER_REV as f32).sin();
        let a = unevenness * crank;
        let mut lp = 0.0f32;
        for j in 0..pulse_len {
            let t = j as f32 / RATE as f32;
            lp += puff_lp * (noise.next() - lp);
            let bark = (TAU * bark_hz * t).sin() * env(t, bark_tau);
            let puff = lp * env(t, puff_tau) * puff_gain;
            v[(start + j) % len] += a * (bark + puff);
        }
    }
    // Body under the pulses, and on load a turbo whine, all integer cycles per loop.
    for (i, s) in v.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        *s += sine(ENGINE_F0, t) * 0.30 + sine(ENGINE_F0 * 0.5, t) * 0.12;
        if load {
            *s += sine(ENGINE_F0 * 27.0, t) * 0.035;
        }
    }
    let cutoff = if load { 0.30 } else { 0.18 };
    let mut lp = 0.0f32;
    for _ in 0..2 {
        for s in v.iter_mut() {
            lp += cutoff * (*s - lp);
            *s = lp;
        }
    }
    let peak = v.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
    for s in v.iter_mut() {
        *s *= 0.85 / peak;
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
    for load in [false, true] {
        let volume = if load { 0.0 } else { 0.12 };
        commands.spawn((AudioPlayer(mk(engine_loop(load))), PlaybackSettings { volume: Volume::Linear(volume), ..PlaybackSettings::LOOP }, Engine { load }));
    }
    commands.insert_resource(sounds);
    commands.insert_resource(Jitter(0));
    commands.insert_resource(Rpm::default());
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
            SimEvent::Rerailed { pos, .. } => (&sounds.clack, 0.8, 0.8, pos),
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

fn engine(sim: Res<Sim>, rig: Res<Rig>, time: Res<Time>, mut rpm: ResMut<Rpm>, mut q: Query<(&mut AudioSink, &Engine)>) {
    let target = (sim.controls.throttle as f32 / 8.0).clamp(0.0, 1.0);
    // Spools up faster than it settles.
    let tau = if target > rpm.0 { 1.2 } else { 2.2 };
    rpm.0 += (target - rpm.0) * (1.0 - (-time.delta_secs() / tau).exp());
    let zoom = (350.0 / (rig.distance + 350.0)).clamp(0.15, 1.0);
    let speed = 1.0 + rpm.0 * 1.35;
    for (mut sink, e) in &mut q {
        sink.set_speed(speed);
        let gain = if e.load { 0.30 * rpm.0.powf(0.7) } else { 0.12 * (1.0 - rpm.0 * 0.6) };
        sink.set_volume(Volume::Linear(gain * zoom));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The loop must join on itself: no step at the seam, and nothing near full scale.
    #[test]
    fn engine_loops_are_seamless_and_unclipped() {
        for load in [false, true] {
            let v = engine_loop(load);
            let seam = (v[0] - v[v.len() - 1]).abs();
            let typical = v.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f32>() / v.len() as f32;
            assert!(seam < typical * 4.0, "load {load}: seam step {seam} vs typical {typical}");
            let peak = v.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(peak <= 0.86 && peak > 0.5, "load {load}: peak {peak}");
        }
    }
}
