//! Game feel: particles, squash & stretch, floating text, flying items,
//! world-anchored UI labels, camera shake and sound playback.

use bevy::audio::Volume;
use bevy::prelude::*;
use bevy::ui::ComputedNode;
use rand::Rng;
use std::collections::HashMap;

use crate::assets::{Art, Sfx};
use crate::data::{icon, Who};
use crate::env::DayNight;
use crate::farming::{Farm, FarmingPlugin, Fx, FxKind, GlowLight};
use crate::world::{tile_bottom, tile_center};
use crate::{MainCamera, Player};

// ------------------------------------------------------------------ sound

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum S {
    Step,
    Hoe,
    Water,
    Plant,
    Harvest,
    Coin,
    Pickup,
    Click,
    Open,
    Close,
    Select,
    Deny,
    Cook,
    LevelUp,
    Quest,
    Meow,
    Yip,
    Purr,
    Splash,
    Place,
    Horn,
    Bird,
}

/// Request to play a sound. `pos` = world position (attenuated by distance to the player).
#[derive(Event, Clone, Copy)]
pub struct Play {
    pub s: S,
    pub vol: f32,
    pub pitch: f32,
    pub pos: Option<Vec2>,
}

impl Play {
    pub fn ui(s: S) -> Self {
        Play { s, vol: 0.6, pitch: 1.0, pos: None }
    }
    pub fn at(s: S, pos: Vec2) -> Self {
        Play { s, vol: 0.7, pitch: 1.0, pos: Some(pos) }
    }
    pub fn vol(mut self, v: f32) -> Self {
        self.vol = v;
        self
    }
    pub fn pitch(mut self, p: f32) -> Self {
        self.pitch = p;
        self
    }
}

#[derive(Resource)]
pub struct AudioSettings {
    pub sfx: bool,
    pub music: bool,
}

fn play_sounds(
    mut commands: Commands,
    mut ev: EventReader<Play>,
    sfx: Res<Sfx>,
    settings: Res<AudioSettings>,
    player: Query<&Transform, With<Player>>,
    mut step_alt: Local<bool>,
    mut played: Local<HashMap<u8, u32>>,
) {
    played.clear();
    let ppos = player.single().map(|t| t.translation.truncate()).unwrap_or_default();
    let mut rng = rand::thread_rng();
    for p in ev.read() {
        if !settings.sfx {
            continue;
        }
        let mut vol = p.vol;
        if let Some(pos) = p.pos {
            let d = pos.distance(ppos);
            vol *= (1.0 - (d - 40.0).max(0.0) / 220.0).clamp(0.0, 1.0);
        }
        if vol < 0.02 {
            continue;
        }
        // avoid stacking the same sound many times in one frame
        let n = played.entry(p.s as u8).or_insert(0);
        *n += 1;
        if *n > 2 {
            continue;
        }
        let h = match p.s {
            S::Step => {
                *step_alt = !*step_alt;
                &sfx.step[*step_alt as usize]
            }
            S::Hoe => &sfx.hoe,
            S::Water => &sfx.water,
            S::Plant => &sfx.plant,
            S::Harvest => &sfx.harvest,
            S::Coin => &sfx.coin,
            S::Pickup => &sfx.pickup,
            S::Click => &sfx.click,
            S::Open => &sfx.open,
            S::Close => &sfx.close,
            S::Select => &sfx.select,
            S::Deny => &sfx.deny,
            S::Cook => &sfx.cook,
            S::LevelUp => &sfx.levelup,
            S::Quest => &sfx.quest,
            S::Meow => &sfx.meow,
            S::Yip => &sfx.yip,
            S::Purr => &sfx.purr,
            S::Splash => &sfx.splash,
            S::Place => &sfx.place,
            S::Horn => &sfx.horn,
            S::Bird => &sfx.birds[rng.gen_range(0..3)],
        };
        let var = if matches!(p.s, S::LevelUp | S::Quest | S::Cook) { 0.0 } else { rng.gen_range(-0.05..0.05) };
        commands.spawn((
            AudioPlayer::new(h.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(vol)).with_speed(p.pitch + var),
        ));
    }
}

// ------------------------------------------------------------------ tweening

/// Squash & stretch "punch" on spawn/change. Overrides the scale while active.
#[derive(Component)]
pub struct Punch {
    pub t: f32,
    pub amp: f32,
}

impl Punch {
    pub fn new(amp: f32) -> Self {
        Punch { t: 0.0, amp }
    }
}

fn punch(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Punch, &mut Transform)>) {
    for (e, mut p, mut tr) in &mut q {
        p.t += time.delta_secs();
        let k = (-p.t * 7.0).exp();
        let s = (p.t * 26.0).sin() * p.amp * k;
        tr.scale = Vec3::new(1.0 - s * 0.6, 1.0 + s, 1.0);
        if p.t > 0.7 {
            tr.scale = Vec3::ONE;
            commands.entity(e).remove::<Punch>();
        }
    }
}

/// Wind sway for plants/trees; `kick` is added when someone brushes past.
#[derive(Component)]
pub struct Sway {
    pub phase: f32,
    pub amp: f32,
}

#[derive(Component, Default)]
pub struct Kick(pub f32);

/// Anything that moves around (player, friends, pets) and brushes plants.
#[derive(Component, Default)]
pub struct Mover {
    pub vel: Vec2,
}

fn sway(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &Sway, &mut Transform, Option<&mut Kick>), Without<Mover>>,
    movers: Query<(&Transform, &Mover)>,
) {
    let t = time.elapsed_secs();
    let ms: Vec<Vec2> = movers
        .iter()
        .filter(|(_, m)| m.vel.length_squared() > 100.0)
        .map(|(tr, _)| tr.translation.truncate())
        .collect();
    for (e, s, mut tr, kick) in &mut q {
        let pos = tr.translation.truncate();
        let mut k = kick.as_ref().map(|k| k.0).unwrap_or(0.0);
        if s.amp > 0.03 && ms.iter().any(|m| (m.x - pos.x).abs() < 9.0 && (m.y - pos.y).abs() < 9.0) {
            k = 1.0;
        }
        let wind = (t * 1.6 + s.phase).sin() * s.amp + (t * 0.43 + s.phase * 0.3).sin() * s.amp * 0.6;
        let brush = k * (t * 22.0).sin() * 0.22;
        tr.rotation = Quat::from_rotation_z(wind + brush);
        if k > 0.0 {
            let nk = (k - time.delta_secs() * 2.5).max(0.0);
            match kick {
                Some(mut kk) => kk.0 = nk,
                None => {
                    commands.entity(e).insert(Kick(nk));
                }
            }
        }
    }
}

// ------------------------------------------------------------------ particles

#[derive(Component)]
pub struct Particle {
    pub vel: Vec2,
    pub life: f32,
    pub max: f32,
    pub gravity: f32,
    pub drag: f32,
    pub floor: Option<f32>,
    pub fade: bool,
    pub shrink: bool,
    pub size: f32,
}

pub fn spawn_particle(
    commands: &mut Commands,
    pos: Vec2,
    z: f32,
    vel: Vec2,
    color: Color,
    size: f32,
    life: f32,
    gravity: f32,
) -> Entity {
    commands
        .spawn((
            Sprite { color, custom_size: Some(Vec2::splat(size)), ..default() },
            Transform::from_xyz(pos.x, pos.y, z),
            Particle { vel, life, max: life, gravity, drag: 1.5, floor: None, fade: true, shrink: true, size },
        ))
        .id()
}

fn particles(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Particle, &mut Transform, &mut Sprite)>) {
    let dt = time.delta_secs();
    for (e, mut p, mut tr, mut s) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        p.vel.y -= p.gravity * dt;
        let drag = (1.0 - p.drag * dt).max(0.0);
        p.vel *= drag;
        tr.translation.x += p.vel.x * dt;
        tr.translation.y += p.vel.y * dt;
        if let Some(f) = p.floor {
            if tr.translation.y < f {
                tr.translation.y = f;
                p.vel.y = -p.vel.y * 0.3;
                p.vel.x *= 0.6;
            }
        }
        let k = p.life / p.max;
        if p.fade {
            let a = s.color.alpha();
            s.color.set_alpha(a.min(k * 1.5).min(1.0));
        }
        if p.shrink {
            s.custom_size = Some(Vec2::splat((p.size * (0.4 + 0.6 * k)).max(1.0)));
        }
    }
}

fn burst(commands: &mut Commands, pos: Vec2, n: usize, colors: &[Color], speed: f32, up: f32, gravity: f32, floor: f32) {
    let mut rng = rand::thread_rng();
    for _ in 0..n {
        let a = rng.gen_range(0.0..std::f32::consts::TAU);
        let v = Vec2::new(a.cos(), a.sin() * 0.6) * rng.gen_range(0.3..1.0) * speed + Vec2::Y * up;
        let c = colors[rng.gen_range(0..colors.len())];
        let e = spawn_particle(commands, pos, 50.0, v, c, rng.gen_range(1.0..2.6), rng.gen_range(0.35..0.8), gravity);
        commands.entity(e).entry::<Particle>().and_modify(move |mut p| p.floor = Some(floor));
    }
}

// ------------------------------------------------------------------ world-anchored UI

/// UI node that follows a world position (crisp text above the world).
#[derive(Component)]
pub struct WorldAnchor {
    pub pos: Vec2,
    pub follow: Option<Entity>,
    pub offset: Vec2,
}

#[derive(Component)]
pub struct FloatText {
    pub t: f32,
    pub life: f32,
    pub rise: f32,
}

pub fn spawn_label(commands: &mut Commands, art: &Art, text: impl Into<String>, color: Color, size: f32, pos: Vec2) -> Entity {
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, ..default() },
            Text::new(text),
            TextFont { font: art.font.clone(), font_size: size, ..default() },
            TextColor(color),
            TextShadow { offset: Vec2::new(1.0, 1.0), color: Color::srgba(0.1, 0.06, 0.14, 0.85) },
            WorldAnchor { pos, follow: None, offset: Vec2::ZERO },
            GlobalZIndex(-5),
            Pickable::IGNORE,
        ))
        .id()
}

pub fn float_text(commands: &mut Commands, art: &Art, text: impl Into<String>, color: Color, pos: Vec2) {
    let e = spawn_label(commands, art, text, color, 15.0, pos);
    commands.entity(e).insert((FloatText { t: 0.0, life: 1.3, rise: 18.0 }, GlobalZIndex(5)));
}

fn world_anchors(
    cam: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_scale: Res<UiScale>,
    targets: Query<&GlobalTransform, Without<WorldAnchor>>,
    mut q: Query<(&WorldAnchor, &mut Node, &ComputedNode, Option<&FloatText>, &mut Visibility)>,
) {
    let Ok((camera, ct)) = cam.single() else {
        return;
    };
    for (a, mut node, comp, ft, mut vis) in &mut q {
        let mut world = a.pos;
        if let Some(f) = a.follow {
            if let Ok(t) = targets.get(f) {
                world = t.translation().truncate() + a.offset;
            }
        }
        if let Some(ft) = ft {
            let k = ft.t / ft.life;
            world.y += ft.rise * (1.0 - (1.0 - k).powi(3));
        }
        let Ok(sp) = camera.world_to_viewport(ct, world.extend(0.0)) else {
            *vis = Visibility::Hidden;
            continue;
        };
        let size = comp.size() * comp.inverse_scale_factor();
        node.left = Val::Px((sp.x / ui_scale.0 - size.x / 2.0).round());
        node.top = Val::Px((sp.y / ui_scale.0 - size.y).round());
        if *vis == Visibility::Hidden && size.x > 0.0 {
            *vis = Visibility::Inherited;
        }
    }
}

fn float_texts(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut FloatText, &mut TextColor, &mut TextShadow)>) {
    for (e, mut f, mut c, mut sh) in &mut q {
        f.t += time.delta_secs();
        let k = f.t / f.life;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        let a = if k > 0.6 { 1.0 - (k - 0.6) / 0.4 } else { 1.0 };
        c.0.set_alpha(a);
        sh.color.set_alpha(a * 0.85);
    }
}

// ------------------------------------------------------------------ flying items

#[derive(Component)]
pub struct FlyItem {
    pub from: Vec2,
    pub target: Entity,
    pub t: f32,
    pub dur: f32,
    pub label: Option<(String, Color)>,
    pub for_player: bool,
    pub delay: f32,
}

#[derive(Resource, Default)]
pub struct Combo {
    pub n: u32,
    pub timer: f32,
}

fn fly_items(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<Art>,
    targets: Query<&GlobalTransform, Without<FlyItem>>,
    mut q: Query<(Entity, &mut FlyItem, &mut Transform, &mut Visibility)>,
    mut combo: ResMut<Combo>,
    mut play: EventWriter<Play>,
) {
    combo.timer -= time.delta_secs();
    if combo.timer <= 0.0 {
        combo.n = 0;
    }
    for (e, mut f, mut tr, mut vis) in &mut q {
        if f.delay > 0.0 {
            f.delay -= time.delta_secs();
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Inherited;
        f.t += time.delta_secs();
        let Ok(target) = targets.get(f.target) else {
            commands.entity(e).despawn();
            continue;
        };
        let to = target.translation().truncate() + Vec2::new(0.0, 14.0);
        let k = (f.t / f.dur).min(1.0);
        // pop up first, then home in
        let e1 = 1.0 - (1.0 - k).powi(3);
        let mut p = f.from.lerp(to, e1 * e1);
        p.y += (k * std::f32::consts::PI).sin() * 26.0;
        tr.translation.x = p.x;
        tr.translation.y = p.y;
        let s = if k < 0.2 { 0.6 + k * 2.5 } else { 1.1 - (k - 0.2) * 0.5 };
        tr.scale = Vec3::splat(s);
        if k >= 1.0 {
            commands.entity(e).despawn();
            if f.for_player {
                combo.n = (combo.n + 1).min(10);
                combo.timer = 1.6;
                let semis = [0, 2, 4, 5, 7, 9, 11, 12, 14, 16, 17][combo.n as usize];
                play.write(Play::ui(S::Pickup).vol(0.5).pitch(2f32.powf(semis as f32 / 12.0)));
            }
            if let Some((text, color)) = f.label.take() {
                float_text(&mut commands, &art, text, color, to + Vec2::new(0.0, 8.0));
            }
        }
    }
}

pub fn spawn_fly(
    commands: &mut Commands,
    art: &Art,
    icon_idx: usize,
    from: Vec2,
    target: Entity,
    label: Option<(String, Color)>,
    for_player: bool,
    delay: f32,
) {
    commands.spawn((
        art.icon(icon_idx),
        Transform::from_xyz(from.x, from.y, 55.0).with_scale(Vec3::splat(0.6)),
        Visibility::Hidden,
        FlyItem { from, target, t: 0.0, dur: 0.55, label, for_player, delay },
    ));
}

// ------------------------------------------------------------------ shake

#[derive(Resource, Default)]
pub struct Shake(pub f32);

// ------------------------------------------------------------------ actor lookup

#[derive(Resource, Default)]
pub struct Actors(pub HashMap<Who, Entity>);

// ------------------------------------------------------------------ fx -> particles/sounds

fn fx_effects(
    mut commands: Commands,
    mut fx: EventReader<Fx>,
    art: Res<Art>,
    actors: Res<Actors>,
    mut play: EventWriter<Play>,
    mut shake: ResMut<Shake>,
) {
    let mut rng = rand::thread_rng();
    for f in fx.read() {
        let c = tile_center(f.tile);
        let b = tile_bottom(f.tile);
        let mine = f.who == Who::You;
        match &f.kind {
            FxKind::Till => {
                let dirt = [Color::srgb(0.48, 0.31, 0.20), Color::srgb(0.36, 0.22, 0.13), Color::srgb(0.58, 0.40, 0.26)];
                burst(&mut commands, c, 12, &dirt, 50.0, 55.0, 260.0, b.y + 2.0);
                let grass = [Color::srgb(0.4, 0.66, 0.3), Color::srgb(0.55, 0.8, 0.35)];
                burst(&mut commands, c, 4, &grass, 40.0, 45.0, 220.0, b.y + 2.0);
                play.write(Play::at(S::Hoe, c).vol(0.6));
                if mine {
                    shake.0 = shake.0.max(0.18);
                }
            }
            FxKind::Plant(_) => {
                let cols = [Color::srgb(0.9, 0.82, 0.6), Color::srgb(0.5, 0.78, 0.35)];
                burst(&mut commands, c, 6, &cols, 25.0, 40.0, 200.0, b.y + 3.0);
                play.write(Play::at(S::Plant, c).vol(0.65));
            }
            FxKind::Water => {
                for _ in 0..14 {
                    let x = c.x + rng.gen_range(-7.0..7.0);
                    let y = c.y + rng.gen_range(8.0..16.0);
                    let e = spawn_particle(
                        &mut commands,
                        Vec2::new(x, y),
                        50.0,
                        Vec2::new(rng.gen_range(-8.0..8.0), rng.gen_range(-30.0..-5.0)),
                        Color::srgba(0.55, 0.8, 1.0, 0.95),
                        rng.gen_range(1.0..2.0),
                        rng.gen_range(0.3..0.55),
                        300.0,
                    );
                    let floor = c.y + rng.gen_range(-6.0..4.0);
                    commands.entity(e).entry::<Particle>().and_modify(move |mut p| p.floor = Some(floor));
                }
                play.write(Play::at(S::Water, c).vol(0.55));
            }
            FxKind::Harvest(crop, n) => {
                let pc = crop.particle_color();
                let cols = [Color::srgb(pc[0], pc[1], pc[2]), Color::srgb(0.55, 0.8, 0.35), Color::srgb(1.0, 0.97, 0.75)];
                burst(&mut commands, c + Vec2::Y * 4.0, 16, &cols, 70.0, 60.0, 240.0, b.y + 2.0);
                for _ in 0..4 {
                    let e = commands
                        .spawn((
                            art.icon(icon::SPARKLE),
                            Transform::from_xyz(c.x + rng.gen_range(-8.0..8.0), c.y + rng.gen_range(0.0..14.0), 52.0)
                                .with_scale(Vec3::splat(0.5)),
                            Particle {
                                vel: Vec2::new(0.0, 12.0),
                                life: 0.5,
                                max: 0.5,
                                gravity: 0.0,
                                drag: 0.0,
                                floor: None,
                                fade: false,
                                shrink: false,
                                size: 16.0,
                            },
                            Punch::new(0.8),
                        ))
                        .id();
                    let _ = e;
                }
                play.write(Play::at(S::Harvest, c).vol(0.7));
                if let Some(t) = actors.0.get(&f.who) {
                    for i in 0..*n {
                        let label = if i + 1 == *n {
                            Some((format!("+{} {}", n, crop.name()), Color::srgb(1.0, 0.95, 0.75)))
                        } else {
                            None
                        };
                        spawn_fly(&mut commands, &art, crop.icon(), c + Vec2::Y * 6.0, *t, label, mine, i as f32 * 0.12);
                    }
                }
                if mine {
                    shake.0 = shake.0.max(0.12);
                }
            }
            FxKind::Place(_) => {
                let cols = [Color::srgba(1.0, 1.0, 1.0, 0.8), Color::srgba(0.9, 0.85, 0.7, 0.8)];
                burst(&mut commands, b + Vec2::Y * 2.0, 14, &cols, 60.0, 10.0, 0.0, b.y - 4.0);
                play.write(Play::at(S::Place, c).vol(0.7));
                if mine {
                    shake.0 = shake.0.max(0.1);
                }
            }
            FxKind::Pickup => {
                let cols = [Color::srgba(1.0, 1.0, 1.0, 0.8)];
                burst(&mut commands, b + Vec2::Y * 4.0, 10, &cols, 50.0, 10.0, 0.0, b.y - 4.0);
                play.write(Play::at(S::Pickup, c).vol(0.6));
            }
            FxKind::Deny(text) => {
                if mine {
                    float_text(&mut commands, &art, text.clone(), Color::srgb(1.0, 0.62, 0.55), c + Vec2::Y * 10.0);
                    play.write(Play::ui(S::Deny).vol(0.4));
                }
            }
            FxKind::Refill => {
                let cols = [Color::srgba(0.6, 0.85, 1.0, 0.9), Color::srgba(1.0, 1.0, 1.0, 0.9)];
                burst(&mut commands, c, 16, &cols, 45.0, 70.0, 260.0, c.y - 6.0);
                play.write(Play::at(S::Splash, c).vol(0.6));
            }
        }
    }
}

/// Ready crops twinkle so you can spot them from afar.
fn ready_sparkles(mut commands: Commands, time: Res<Time>, farm: Res<Farm>, art: Res<Art>, mut acc: Local<f32>) {
    *acc += time.delta_secs();
    if *acc < 0.25 {
        return;
    }
    *acc = 0.0;
    let mut rng = rand::thread_rng();
    let ready: Vec<IVec2> = farm.plots.iter().filter(|(_, p)| p.ready()).map(|(k, _)| IVec2::new(k.0, k.1)).collect();
    if ready.is_empty() {
        return;
    }
    let n = (ready.len() / 6).clamp(1, 4);
    for _ in 0..n {
        let p = ready[rng.gen_range(0..ready.len())];
        let c = tile_center(p) + Vec2::new(rng.gen_range(-6.0..6.0), rng.gen_range(0.0..12.0));
        commands.spawn((
            art.icon(icon::SPARKLE),
            Transform::from_xyz(c.x, c.y, 52.0).with_scale(Vec3::splat(0.35)),
            Particle { vel: Vec2::new(0.0, 6.0), life: 0.6, max: 0.6, gravity: 0.0, drag: 0.0, floor: None, fade: false, shrink: false, size: 16.0 },
            Punch::new(0.9),
        ));
    }
}

fn glow_lights(time: Res<Time>, dn: Res<DayNight>, mut q: Query<(&GlowLight, &mut Sprite)>) {
    let t = time.elapsed_secs();
    for (g, mut s) in &mut q {
        let flicker = 0.9 + 0.1 * (t * 7.0 + g.phase).sin() * (t * 3.1 + g.phase * 2.0).sin();
        s.color.set_alpha(g.base * dn.darkness.powf(0.7) * flicker * 1.4);
    }
}

pub struct JuicePlugin;

impl Plugin for JuicePlugin {
    fn build(&self, app: &mut App) {
        let _ = FarmingPlugin;
        app.add_event::<Play>()
            .insert_resource(AudioSettings { sfx: true, music: true })
            .init_resource::<Combo>()
            .init_resource::<Shake>()
            .init_resource::<Actors>()
            .add_systems(
                Update,
                (play_sounds, punch, sway, particles, fx_effects, fly_items, float_texts, ready_sparkles, glow_lights),
            )
            .add_systems(PostUpdate, world_anchors.before(bevy::ui::UiSystem::Layout));
    }
}
