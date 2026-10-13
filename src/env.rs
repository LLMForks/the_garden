//! Time of day, seasons, weather, ambient visuals and audio.

use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::assets::{Art, Sfx};
use crate::data::*;
use crate::farming::Farm;
use crate::juice::{spawn_particle, AudioSettings, Particle, Play, S};
use crate::state::Note;
use crate::world::{KitchenSprite, Island};
use crate::{GameSpeed, MainCamera, Paused, TILE};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Weather {
    Clear,
    Rain,
    Snow,
}

#[derive(Resource, Clone, Serialize, Deserialize, Debug)]
pub struct SeasonClock {
    pub day: u32,
    pub t: f32,
    pub weather: Weather,
}

impl Default for SeasonClock {
    fn default() -> Self {
        SeasonClock { day: 0, t: DAY_SECS * 0.22, weather: Weather::Clear }
    }
}

impl SeasonClock {
    pub fn season(&self) -> Season {
        Season::ALL[((self.day / DAYS_PER_SEASON) % 4) as usize]
    }
    pub fn day_in_season(&self) -> u32 {
        self.day % DAYS_PER_SEASON + 1
    }
    pub fn year(&self) -> u32 {
        self.day / (DAYS_PER_SEASON * 4) + 1
    }
    /// 0..1 through the day
    pub fn k(&self) -> f32 {
        self.t / DAY_SECS
    }
    pub fn clock_text(&self) -> String {
        // nonlinear clock: long days, short nights
        let k = self.k();
        let pts = [(0.0, 6.0), (0.06, 7.0), (0.74, 19.0), (0.84, 22.0), (0.98, 29.0), (1.0, 30.0)];
        let mut h = 6.0;
        for w in pts.windows(2) {
            let ((k0, h0), (k1, h1)) = (w[0], w[1]);
            if k >= k0 && k <= k1 {
                h = h0 + (h1 - h0) * (k - k0) / (k1 - k0);
            }
        }
        let mins = ((h * 60.0) as u32) % (24 * 60);
        format!("{:02}:{:02}", mins / 60, (mins % 60) / 10 * 10)
    }
    pub fn darkness(&self) -> f32 {
        let k = self.k();
        let ss = |a: f32, b: f32, x: f32| {
            let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        // dusk 0.74 -> 0.84, dawn 0.98 -> 0.06 (wrapping)
        if k >= 0.6 {
            ss(0.74, 0.84, k)
        } else {
            1.0 - ss(-0.02, 0.06, k)
        }
    }
    pub fn dusk(&self) -> f32 {
        let k = self.k();
        let d = 1.0 - ((k - 0.76) / 0.06).abs().min(1.0);
        let m = 1.0 - ((k - 0.02) / 0.05).abs().min(1.0);
        d.max(m * 0.7)
    }
}

#[derive(Resource, Default)]
pub struct DayNight {
    pub darkness: f32,
}

#[derive(Component)]
struct Overlay;
#[derive(Component)]
struct Cloud {
    speed: f32,
}
#[derive(Component)]
struct Music;
#[derive(Component)]
struct Waves;
#[derive(Component)]
struct Crickets;

pub struct EnvPlugin;

impl Plugin for EnvPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SeasonClock>()
            .init_resource::<DayNight>()
            .add_systems(Startup, setup_env)
            .add_systems(Update, (tick_clock, overlay, clouds, weather_particles, ambience, kitchen_windows));
    }
}

fn setup_env(mut commands: Commands, art: Res<Art>, sfx: Res<Sfx>, island: Res<Island>) {
    commands.spawn((
        Sprite { color: Color::NONE, custom_size: Some(Vec2::splat(6000.0)), ..default() },
        Transform::from_xyz(0.0, 0.0, 60.0),
        Overlay,
    ));
    let mut rng = rand::thread_rng();
    for _ in 0..5 {
        let x = rng.gen_range(0.0..island.w as f32 * TILE);
        let y = -rng.gen_range(0.0..island.h as f32 * TILE);
        let s = rng.gen_range(1.4..2.6);
        commands.spawn((
            Sprite { color: Color::srgba(1.0, 1.0, 1.0, 0.09), ..Sprite::from_image(art.cloud.clone()) },
            Transform::from_xyz(x, y, 59.0).with_scale(Vec3::new(s, s * 0.8, 1.0)),
            Cloud { speed: rng.gen_range(5.0..10.0) },
        ));
    }
    commands.spawn((
        AudioPlayer::new(sfx.music.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        Music,
    ));
    commands.spawn((
        AudioPlayer::new(sfx.waves.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        Waves,
    ));
    commands.spawn((
        AudioPlayer::new(sfx.night.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        Crickets,
    ));
}

fn tick_clock(
    time: Res<Time>,
    speed: Res<GameSpeed>,
    paused: Res<Paused>,
    mut clock: ResMut<SeasonClock>,
    mut dn: ResMut<DayNight>,
    mut farm: ResMut<Farm>,
    mut notes: EventWriter<Note>,
    mut rain_tick: Local<f32>,
) {
    if paused.0 {
        return;
    }
    clock.t += time.delta_secs() * speed.0;
    if clock.t >= DAY_SECS {
        clock.t -= DAY_SECS;
        let old = clock.season();
        clock.day += 1;
        let mut rng = rand::thread_rng();
        let s = clock.season();
        clock.weather = match s {
            Season::Winter => {
                if rng.gen::<f32>() < 0.55 {
                    Weather::Snow
                } else {
                    Weather::Clear
                }
            }
            _ => {
                if rng.gen::<f32>() < 0.28 {
                    Weather::Rain
                } else {
                    Weather::Clear
                }
            }
        };
        // a new day: the soil dries (nothing withers, it just grows slower)
        let rain = clock.weather == Weather::Rain;
        for p in farm.plots.values_mut() {
            p.watered = rain;
        }
        if s != old {
            notes.write(Note::Toast(format!("Наступила {}! {}", s.name().to_lowercase(), s.bonus_text()), icon::STAR));
        } else {
            notes.write(Note::Feed(format!("День {} · {}", clock.day_in_season(), s.name())));
        }
        if rain {
            notes.write(Note::Toast("Идёт дождь — грядки поливаются сами".into(), icon::DROP));
        }
    }
    if clock.weather == Weather::Rain {
        *rain_tick += time.delta_secs();
        if *rain_tick > 1.0 {
            *rain_tick = 0.0;
            for p in farm.plots.values_mut() {
                p.watered = true;
            }
        }
    }
    dn.darkness = clock.darkness();
}

fn overlay(
    clock: Res<SeasonClock>,
    dn: Res<DayNight>,
    cam: Query<&Transform, (With<MainCamera>, Without<Overlay>)>,
    mut q: Query<(&mut Transform, &mut Sprite), With<Overlay>>,
) {
    let Ok(ct) = cam.single() else { return };
    let Ok((mut tr, mut s)) = q.single_mut() else { return };
    tr.translation.x = ct.translation.x;
    tr.translation.y = ct.translation.y;
    let d = dn.darkness;
    let dusk = clock.dusk();
    let night = Vec4::new(0.06, 0.07, 0.22, 0.46);
    let eve = Vec4::new(0.95, 0.45, 0.25, 0.16);
    let mut c = Vec4::ZERO;
    c = c.lerp(eve, dusk * (1.0 - d * 0.6));
    c = c.lerp(night, d);
    if clock.weather == Weather::Rain {
        c = c.lerp(Vec4::new(0.25, 0.30, 0.42, 0.22), 0.6 * (1.0 - d));
    }
    if clock.season() == Season::Winter {
        c = c.lerp(Vec4::new(0.6, 0.7, 1.0, 0.07), 0.5 * (1.0 - d));
    }
    s.color = Color::srgba(c.x, c.y, c.z, c.w);
}

fn clouds(time: Res<Time>, island: Res<Island>, clock: Res<SeasonClock>, mut q: Query<(&mut Transform, &mut Sprite, &Cloud)>) {
    let w = island.w as f32 * TILE;
    let a = if clock.weather == Weather::Rain { 0.16 } else { 0.08 };
    for (mut t, mut s, c) in &mut q {
        t.translation.x += c.speed * time.delta_secs();
        t.translation.y -= c.speed * 0.25 * time.delta_secs();
        if t.translation.x > w + 250.0 {
            t.translation.x = -250.0;
            t.translation.y = -rand::thread_rng().gen_range(0.0..island.h as f32 * TILE);
        }
        s.color = Color::srgba(1.0, 1.0, 1.0, a);
    }
}

fn weather_particles(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<SeasonClock>,
    cam: Query<(&Transform, &Projection), With<MainCamera>>,
    mut acc: Local<f32>,
) {
    let Ok((ct, proj)) = cam.single() else { return };
    let scale = match proj {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    };
    let half = Vec2::new(700.0, 420.0) * scale;
    let c = ct.translation.truncate();
    let mut rng = rand::thread_rng();
    *acc += time.delta_secs();
    let (rate, kind) = match (clock.weather, clock.season()) {
        (Weather::Rain, _) => (220.0, 0),
        (Weather::Snow, _) => (40.0, 1),
        (_, Season::Winter) => (8.0, 1),
        (_, Season::Autumn) => (5.0, 2),
        (_, Season::Spring) => (4.0, 3),
        _ => (0.0, 9),
    };
    if rate == 0.0 {
        *acc = 0.0;
        return;
    }
    let n = (*acc * rate) as usize;
    if n == 0 {
        return;
    }
    *acc -= n as f32 / rate;
    for _ in 0..n.min(30) {
        let x = c.x + rng.gen_range(-half.x..half.x);
        let y = c.y + half.y + rng.gen_range(0.0..40.0);
        match kind {
            0 => {
                let e = commands
                    .spawn((
                        Sprite {
                            color: Color::srgba(0.75, 0.85, 1.0, 0.55),
                            custom_size: Some(Vec2::new(1.0, 5.0)),
                            ..default()
                        },
                        Transform::from_xyz(x, c.y + rng.gen_range(-half.y..half.y) + 60.0, 58.0)
                            .with_rotation(Quat::from_rotation_z(0.12)),
                        Particle {
                            vel: Vec2::new(-30.0, -280.0),
                            life: rng.gen_range(0.15..0.35),
                            max: 0.35,
                            gravity: 0.0,
                            drag: 0.0,
                            floor: None,
                            fade: false,
                            shrink: false,
                            size: 1.0,
                        },
                    ))
                    .id();
                let _ = e;
            }
            1 => {
                let e = spawn_particle(
                    &mut commands,
                    Vec2::new(x, y),
                    58.0,
                    Vec2::new(rng.gen_range(-12.0..4.0), rng.gen_range(-28.0..-16.0)),
                    Color::srgba(1.0, 1.0, 1.0, 0.9),
                    rng.gen_range(1.0..2.2),
                    rng.gen_range(10.0..16.0),
                    0.0,
                );
                commands.entity(e).entry::<Particle>().and_modify(|mut p| {
                    p.drag = 0.0;
                    p.shrink = false;
                });
            }
            _ => {
                let col = if kind == 2 {
                    [Color::srgb(0.9, 0.5, 0.2), Color::srgb(0.85, 0.35, 0.2), Color::srgb(0.95, 0.72, 0.3)][rng.gen_range(0..3)]
                } else {
                    [Color::srgb(1.0, 0.8, 0.87), Color::srgb(1.0, 1.0, 1.0)][rng.gen_range(0..2)]
                };
                let e = spawn_particle(
                    &mut commands,
                    Vec2::new(x, y),
                    58.0,
                    Vec2::new(rng.gen_range(-20.0..-5.0), rng.gen_range(-22.0..-12.0)),
                    col,
                    2.0,
                    rng.gen_range(12.0..18.0),
                    0.0,
                );
                commands.entity(e).entry::<Particle>().and_modify(|mut p| {
                    p.drag = 0.0;
                    p.shrink = false;
                });
            }
        }
    }
}

fn ambience(
    time: Res<Time>,
    clock: Res<SeasonClock>,
    settings: Res<AudioSettings>,
    mut music: Query<&mut AudioSink, (With<Music>, Without<Waves>, Without<Crickets>)>,
    mut waves: Query<&mut AudioSink, (With<Waves>, Without<Music>, Without<Crickets>)>,
    mut crickets: Query<&mut AudioSink, (With<Crickets>, Without<Music>, Without<Waves>)>,
    mut play: EventWriter<Play>,
    mut fade: Local<f32>,
    mut bird_t: Local<f32>,
) {
    *fade = (*fade + time.delta_secs() * 0.3).min(1.0);
    let d = clock.darkness();
    if let Ok(mut s) = music.single_mut() {
        let v = if settings.music { 0.45 * *fade } else { 0.0 };
        s.set_volume(Volume::Linear(v));
    }
    let amb = if settings.sfx { *fade } else { 0.0 };
    if let Ok(mut s) = waves.single_mut() {
        s.set_volume(Volume::Linear(0.22 * amb));
    }
    if let Ok(mut s) = crickets.single_mut() {
        let night = if clock.season() == Season::Winter { 0.0 } else { d };
        s.set_volume(Volume::Linear(0.25 * night * amb));
    }
    *bird_t -= time.delta_secs();
    if *bird_t <= 0.0 {
        let mut rng = rand::thread_rng();
        *bird_t = rng.gen_range(5.0..13.0);
        if d < 0.3 && clock.season() != Season::Winter && clock.weather != Weather::Rain {
            play.write(Play::ui(S::Bird).vol(0.18).pitch(rng.gen_range(0.9..1.15)));
        }
    }
}

fn kitchen_windows(dn: Res<DayNight>, mut q: Query<&mut Sprite, With<KitchenSprite>>) {
    for mut s in &mut q {
        if let Some(a) = s.texture_atlas.as_mut() {
            let want = (dn.darkness > 0.35) as usize;
            if a.index != want {
                a.index = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seasons_cycle_and_never_end() {
        let mut c = SeasonClock::default();
        let mut seen = vec![];
        for d in 0..(DAYS_PER_SEASON * 4 * 2) {
            c.day = d;
            seen.push(c.season());
        }
        assert_eq!(seen[0], Season::Spring);
        assert_eq!(seen[(DAYS_PER_SEASON * 4) as usize], Season::Spring, "next year starts again");
        assert!(seen.contains(&Season::Winter));
    }

    #[test]
    fn night_is_dark_and_noon_is_bright() {
        let mut c = SeasonClock::default();
        c.t = DAY_SECS * 0.45;
        assert!(c.darkness() < 0.01);
        c.t = DAY_SECS * 0.92;
        assert!(c.darkness() > 0.99);
        c.t = DAY_SECS * 0.01;
        assert!(c.darkness() > 0.3);
    }
}
