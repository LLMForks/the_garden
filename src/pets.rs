//! Pets and wildlife attracted by a cozy garden. No care needed (anti-FOMO):
//! they just come, wander around, nap and enjoy being petted.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use rand::seq::SliceRandom;
use rand::Rng;

use crate::assets::Art;
use crate::data::*;
use crate::env::{DayNight, SeasonClock};
use crate::farming::DecorMap;
use crate::juice::{spawn_particle, Mover, Play, S};
use crate::player::{move_with_collision, Interact};
use crate::state::{Game, Note, PetKind};
use crate::world::{tile_center, world_to_tile, Island};
use crate::{ysort, Player};

#[derive(Component)]
pub struct Pet {
    pub kind: PetKind,
    goal: Option<Vec2>,
    t: f32,
    mode: u8, // 0 walk, 1 sit, 2 sleep
    flip: bool,
    anim: f32,
}

#[derive(Component)]
struct Firefly {
    home: Vec2,
    phase: f32,
}

#[derive(Component)]
struct Butterfly {
    home: Vec2,
    phase: f32,
    color: usize,
}

pub struct PetsPlugin;

impl Plugin for PetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (attract_pets, pet_ai, petting, fireflies, butterflies));
    }
}

fn spawn_pet(commands: &mut Commands, art: &Art, kind: PetKind, pos: Vec2) {
    let img = match kind {
        PetKind::Fox => art.fox.clone(),
        _ => art.cat.clone(),
    };
    let tint = if kind == PetKind::Cat2 { Color::srgb(0.75, 0.75, 0.85) } else { Color::WHITE };
    commands
        .spawn((
            Sprite {
                anchor: Anchor::Custom(Vec2::new(0.0, -0.5 + 1.0 / 16.0)),
                color: tint,
                ..Sprite::from_atlas_image(img, TextureAtlas { layout: art.pet_layout.clone(), index: 0 })
            },
            Transform::from_xyz(pos.x, pos.y, ysort(pos.y)),
            Pet { kind, goal: None, t: 1.0, mode: 0, flip: false, anim: 0.0 },
            Mover::default(),
        ))
        .with_child((
            Sprite { custom_size: Some(Vec2::new(10.0, 4.0)), ..Sprite::from_image(art.shadow.clone()) },
            Transform::from_xyz(0.0, 1.0, -0.05),
        ));
}

fn attract_pets(
    mut commands: Commands,
    art: Res<Art>,
    mut game: ResMut<Game>,
    island: Res<Island>,
    pets: Query<&Pet>,
    mut notes: EventWriter<Note>,
    mut play: EventWriter<Play>,
) {
    // respawn pets from a save
    let spawned: Vec<PetKind> = pets.iter().map(|p| p.kind).collect();
    let shared = island.zone(Owner::Shared).clone();
    let home = tile_center((shared.min + shared.max) / 2);
    for k in game.pets.clone() {
        if !spawned.contains(&k) {
            spawn_pet(&mut commands, &art, k, home);
        }
    }
    for k in [PetKind::Cat, PetKind::Fox, PetKind::Cat2] {
        if !game.pets.contains(&k) && game.cozy >= k.cozy_needed() {
            game.pets.push(k);
            // walks in from the beach
            let dock = *island.dock.first().unwrap();
            spawn_pet(&mut commands, &art, k, tile_center(dock - IVec2::Y));
            let text = format!("Уют привлёк гостя: {} теперь живёт на острове!", k.name());
            notes.write(Note::Toast(text, icon::HEART));
            play.write(Play::ui(if k == PetKind::Fox { S::Yip } else { S::Meow }).vol(0.6));
            play.write(Play::ui(S::Quest).vol(0.5));
            break;
        }
    }
}

fn pet_ai(
    time: Res<Time>,
    island: Res<Island>,
    dm: Res<DecorMap>,
    dn: Res<DayNight>,
    player: Query<&Transform, (With<Player>, Without<Pet>)>,
    mut q: Query<(&mut Pet, &mut Transform, &mut Sprite, &mut Mover)>,
) {
    let Ok(pt) = player.single() else { return };
    let ppos = pt.translation.truncate();
    let dt = time.delta_secs().min(0.05);
    let mut rng = rand::thread_rng();
    let spots: Vec<Vec2> = dm
        .items
        .iter()
        .filter(|(_, (d, _))| matches!(d, Decor::Bowl | Decor::Bench | Decor::Lantern | Decor::Flowerbed | Decor::Mushroom))
        .map(|(k, _)| tile_center(IVec2::new(k.0, k.1)))
        .collect();
    for (mut pet, mut tr, mut s, mut mover) in &mut q {
        pet.t -= dt;
        let pos = tr.translation.truncate();
        if pet.t <= 0.0 {
            let roll: f32 = rng.gen();
            if dn.darkness > 0.6 && roll < 0.7 {
                pet.mode = 2;
                pet.goal = None;
                pet.t = rng.gen_range(6.0..12.0);
            } else if roll < 0.3 {
                pet.mode = 1;
                pet.goal = None;
                pet.t = rng.gen_range(2.0..5.0);
            } else {
                pet.mode = 0;
                let near_player = ppos.distance(pos) < 140.0 && rng.gen::<f32>() < 0.45;
                let g = if near_player {
                    ppos + Vec2::new(rng.gen_range(-20.0..20.0), rng.gen_range(-14.0..6.0))
                } else if let Some(s) = spots.choose(&mut rng) {
                    *s + Vec2::new(rng.gen_range(-24.0..24.0), rng.gen_range(-24.0..24.0))
                } else {
                    pos + Vec2::new(rng.gen_range(-60.0..60.0), rng.gen_range(-60.0..60.0))
                };
                pet.goal = if island.walkable(world_to_tile(g)) { Some(g) } else { None };
                pet.t = rng.gen_range(3.0..6.0);
            }
        }
        let mut vel = Vec2::ZERO;
        if let (0, Some(g)) = (pet.mode, pet.goal) {
            let d = g - pos;
            if d.length() > 3.0 {
                vel = d.normalize() * 32.0;
            } else {
                pet.goal = None;
                pet.mode = 1;
                pet.t = rng.gen_range(1.5..4.0);
            }
        }
        mover.vel = mover.vel.lerp(vel, (dt * 8.0).min(1.0));
        let np = move_with_collision(&island, pos, mover.vel * dt);
        if np.distance(pos) < 0.001 && vel != Vec2::ZERO {
            pet.goal = None;
        }
        tr.translation.x = np.x;
        tr.translation.y = np.y;
        tr.translation.z = ysort(np.y);
        if mover.vel.x.abs() > 2.0 {
            pet.flip = mover.vel.x < 0.0;
        }
        s.flip_x = pet.flip;
        pet.anim += dt * 8.0;
        let frame = if mover.vel.length() > 5.0 {
            (pet.anim as usize) % 4
        } else if pet.mode == 2 {
            5
        } else {
            4
        };
        if let Some(a) = s.texture_atlas.as_mut() {
            a.index = frame;
        }
        // breathing while asleep
        let b = if pet.mode == 2 { 1.0 + (time.elapsed_secs() * 2.0).sin() * 0.04 } else { 1.0 };
        tr.scale = Vec3::new(1.0, b, 1.0);
    }
}

fn petting(
    mut commands: Commands,
    mut ev: EventReader<Interact>,
    mut q: Query<(&mut Pet, &Transform)>,
    mut play: EventWriter<Play>,
    mut notes: EventWriter<Note>,
    mut last: Local<f32>,
    time: Res<Time>,
) {
    for i in ev.read() {
        if let Interact::Pet(e) = i {
            if let Ok((mut pet, tr)) = q.get_mut(*e) {
                pet.mode = 1;
                pet.goal = None;
                pet.t = 3.0;
                let mut rng = rand::thread_rng();
                for k in 0..5 {
                    let _ = spawn_particle(
                        &mut commands,
                        tr.translation.truncate() + Vec2::new(k as f32 * 3.0 - 6.0, 12.0),
                        55.0,
                        Vec2::new(rng.gen_range(-10.0..10.0), rng.gen_range(16.0..30.0)),
                        Color::srgb(1.0, 0.45, 0.6),
                        3.0,
                        1.0,
                        0.0,
                    );
                }
                play.write(Play::ui(S::Purr).vol(0.7));
                play.write(Play::ui(if pet.kind == PetKind::Fox { S::Yip } else { S::Meow }).vol(0.5));
                if time.elapsed_secs() - *last > 20.0 {
                    *last = time.elapsed_secs();
                    let mut n = pet.kind.name().chars();
                    let cap: String = n.next().map(|c| c.to_uppercase().collect::<String>() + n.as_str()).unwrap_or_default();
                    notes.write(Note::Feed(format!("{} довольно щурится", cap)));
                }
            }
        }
    }
}

fn fireflies(
    mut commands: Commands,
    art: Res<Art>,
    time: Res<Time>,
    dn: Res<DayNight>,
    game: Res<Game>,
    dm: Res<DecorMap>,
    clock: Res<SeasonClock>,
    mut q: Query<(Entity, &Firefly, &mut Transform, &mut Sprite, &mut crate::env::LightSource)>,
) {
    let want = if clock.season() == Season::Winter || game.cozy < 3 {
        0
    } else {
        (game.cozy as usize * 2).min(36)
    };
    let have = q.iter().count();
    let mut rng = rand::thread_rng();
    if have < want && !dm.items.is_empty() {
        let spots: Vec<Vec2> = dm.items.keys().map(|k| tile_center(IVec2::new(k.0, k.1))).collect();
        let home = *spots.choose(&mut rng).unwrap() + Vec2::new(rng.gen_range(-30.0..30.0), rng.gen_range(-20.0..30.0));
        commands.spawn((
            Sprite {
                color: Color::srgba(0.85, 1.0, 0.45, 0.0),
                custom_size: Some(Vec2::splat(6.0)),
                ..Sprite::from_image(art.light.clone())
            },
            Transform::from_xyz(home.x, home.y, 62.0),
            Firefly { home, phase: rng.gen_range(0.0..100.0) },
            crate::env::LightSource { radius: 10.0, intensity: 0.0, flicker: false, phase: 0.0 },
        ));
    } else if have > want {
        if let Some((e, ..)) = q.iter().next() {
            commands.entity(e).despawn();
        }
    }
    let t = time.elapsed_secs();
    let night = ((dn.darkness - 0.4) / 0.5).clamp(0.0, 1.0);
    for (_, f, mut tr, mut s, mut light) in &mut q {
        let p = f.home
            + Vec2::new((t * 0.5 + f.phase).sin() * 22.0 + (t * 1.3 + f.phase).cos() * 6.0, (t * 0.7 + f.phase * 1.7).sin() * 14.0);
        tr.translation.x = p.x;
        tr.translation.y = p.y;
        let blink = ((t * 2.2 + f.phase).sin() * 0.5 + 0.5).powf(2.0);
        s.color.set_alpha(night * (0.35 + 0.65 * blink));
        light.intensity = 0.5 * blink;
    }
}

fn butterflies(
    mut commands: Commands,
    art: Res<Art>,
    time: Res<Time>,
    dn: Res<DayNight>,
    dm: Res<DecorMap>,
    clock: Res<SeasonClock>,
    player: Query<&Transform, (With<Player>, Without<Butterfly>)>,
    mut q: Query<(Entity, &mut Butterfly, &mut Transform, &mut Sprite)>,
) {
    let flowers = dm.items.values().filter(|(d, _)| matches!(d, Decor::Flowerbed | Decor::Sunflower)).count();
    let base = match clock.season() {
        Season::Spring | Season::Summer => 5,
        Season::Autumn => 2,
        Season::Winter => 0,
    };
    let want = if dn.darkness > 0.3 || clock.weather != crate::env::Weather::Clear { 0 } else { base + flowers * 2 };
    let have = q.iter().count();
    let mut rng = rand::thread_rng();
    let Ok(pt) = player.single() else { return };
    if have < want.min(18) {
        let home = if flowers > 0 && rng.gen::<f32>() < 0.6 {
            let f: Vec<Vec2> = dm
                .items
                .iter()
                .filter(|(_, (d, _))| matches!(d, Decor::Flowerbed | Decor::Sunflower))
                .map(|(k, _)| tile_center(IVec2::new(k.0, k.1)))
                .collect();
            *f.choose(&mut rng).unwrap()
        } else {
            pt.translation.truncate() + Vec2::new(rng.gen_range(-200.0..200.0), rng.gen_range(-130.0..130.0))
        };
        let color = rng.gen_range(0..4);
        commands.spawn((
            Sprite::from_atlas_image(art.butterfly.clone(), TextureAtlas { layout: art.butterfly_layout.clone(), index: color * 2 }),
            Transform::from_xyz(home.x, home.y + 10.0, 57.0),
            Butterfly { home, phase: rng.gen_range(0.0..100.0), color },
        ));
    } else if have > want {
        if let Some((e, ..)) = q.iter().next() {
            commands.entity(e).despawn();
        }
    }
    let t = time.elapsed_secs();
    for (_, b, mut tr, mut s) in &mut q {
        let p = b.home
            + Vec2::new((t * 0.6 + b.phase).sin() * 26.0 + (t * 1.9 + b.phase).sin() * 5.0, (t * 0.9 + b.phase).cos() * 12.0 + 10.0);
        let vx = p.x - tr.translation.x;
        tr.translation.x = p.x;
        tr.translation.y = p.y + ((t * 9.0 + b.phase).sin() * 1.5);
        s.flip_x = vx < 0.0;
        if let Some(a) = s.texture_atlas.as_mut() {
            a.index = b.color * 2 + (((t * 10.0 + b.phase) as usize) % 2);
        }
    }
}
