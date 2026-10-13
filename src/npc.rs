//! Friends who share the island. They drop in and out by boat, tend their own
//! personal zones, water other people's plots, cook from surplus and gift decor.
//! This stands in for real co-op players in the offline beta.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use rand::seq::SliceRandom;
use rand::Rng;

use crate::assets::Art;
use crate::data::*;
use crate::farming::{self, DecorMap, Farm, Fx, FxKind};
use crate::juice::{Actors, Mover, Play, WorldAnchor, S};
use crate::player::{move_with_collision, spawn_character, CharAnim, Interact, Talkable};
use crate::state::{Game, Note};
use crate::world::{tile_center, world_to_tile, Building, Island};
use crate::{ysort, GameSpeed, Paused, Player};

#[derive(Clone, Debug)]
enum Task {
    Till(IVec2),
    Plant(IVec2, Crop),
    Water(IVec2),
    Harvest(IVec2),
    Cook(Dish),
    PlaceDecor(IVec2, Decor),
    Gift(Decor),
    Wander,
    Leave,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Away,
    Idle,
    Walking,
    Working,
}

#[derive(Component)]
pub struct Npc {
    pub who: Who,
    phase: Phase,
    task: Option<Task>,
    path: Vec<IVec2>,
    wait: f32,
    work_t: f32,
    /// seconds left on the island (or until arrival when away)
    stay: f32,
    chat_t: f32,
    cook_cd: f32,
    decor_cd: f32,
    gift_cd: f32,
    feed_cd: f32,
    stuck: f32,
}

#[derive(Component)]
pub struct Boat {
    t: f32,
    who: Who,
    leaving: bool,
}

#[derive(Component)]
pub struct Bubble {
    t: f32,
}

#[derive(Event)]
pub struct Say(pub Who, pub String);

pub struct NpcPlugin;

impl Plugin for NpcPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<Say>()
            .add_systems(Startup, spawn_npcs)
            .add_systems(Update, (npc_brain, npc_move, boats, talk, bubbles, chatter));
    }
}

fn spawn_npcs(mut commands: Commands, art: Res<Art>, island: Res<Island>, mut actors: ResMut<Actors>) {
    for (i, who) in Who::FRIENDS.iter().enumerate() {
        let z = island.zone(who.zone());
        let home = IVec2::new((z.min.x + z.max.x) / 2, (z.min.y + z.max.y) / 2);
        let present = i == 0;
        let pos = if present { tile_center(home) } else { Vec2::new(-5000.0, 5000.0) };
        let e = spawn_character(&mut commands, &art, *who, pos);
        commands.entity(e).insert((
            Npc {
                who: *who,
                phase: if present { Phase::Idle } else { Phase::Away },
                task: None,
                path: vec![],
                wait: 1.0 + i as f32,
                work_t: 0.0,
                stay: if present { 260.0 } else { 35.0 + 50.0 * i as f32 },
                chat_t: 12.0 + 7.0 * i as f32,
                cook_cd: 40.0,
                decor_cd: 30.0,
                gift_cd: 120.0,
                feed_cd: 0.0,
                stuck: 0.0,
            },
            Talkable(*who),
            if present { Visibility::Inherited } else { Visibility::Hidden },
        ));
        actors.0.insert(*who, e);
    }
}

fn zone_tiles(isl: &Island, who: Who) -> Vec<IVec2> {
    let z = isl.zone(who.zone());
    let mut v = vec![];
    for y in z.min.y..=z.max.y {
        for x in z.min.x..=z.max.x {
            v.push(IVec2::new(x, y));
        }
    }
    v
}

/// Nice, tidy garden beds near the friend's tent.
fn next_bed_tile(isl: &Island, farm: &Farm, dm: &DecorMap, who: Who) -> Option<IVec2> {
    let z = isl.zone(who.zone());
    let (tent_p, _) = isl.building(Building::Tent(who));
    let mut best: Option<(i32, IVec2)> = None;
    for y in (z.min.y + 1)..=(z.max.y - 1) {
        for x in (z.min.x + 1)..=(z.max.x - 1) {
            let p = IVec2::new(x, y);
            if y % 3 == 0 {
                continue; // leave walkways between beds
            }
            if !isl.farmable(p) || farm.get(p).is_some() || dm.items.contains_key(&(x, y)) {
                continue;
            }
            let d = (p - tent_p).abs().element_sum();
            if (p - tent_p).abs().max_element() < 3 {
                continue;
            }
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, p));
            }
        }
    }
    best.map(|b| b.1)
}

fn stand_tile(isl: &Island, t: IVec2) -> IVec2 {
    for d in [IVec2::new(0, 1), IVec2::new(-1, 0), IVec2::new(1, 0), IVec2::new(0, -1)] {
        if isl.walkable(t + d) {
            return t + d;
        }
    }
    t
}

fn task_target(isl: &Island, task: &Task, who: Who, rng: &mut impl Rng) -> Option<IVec2> {
    match task {
        Task::Till(p) | Task::Plant(p, _) | Task::Water(p) | Task::Harvest(p) | Task::PlaceDecor(p, _) => {
            Some(stand_tile(isl, *p))
        }
        Task::Cook(_) => Some(isl.building_front(Building::Kitchen)),
        Task::Gift(_) => Some(isl.building_front(Building::Chest)),
        Task::Leave => isl.dock.last().copied(),
        Task::Wander => {
            let zone = if rng.gen::<f32>() < 0.5 { Owner::Shared } else { who.zone() };
            let z = isl.zone(zone);
            for _ in 0..20 {
                let p = IVec2::new(rng.gen_range(z.min.x..=z.max.x), rng.gen_range(z.min.y..=z.max.y));
                if isl.walkable(p) {
                    return Some(p);
                }
            }
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn choose_task(
    npc: &mut Npc,
    isl: &Island,
    farm: &Farm,
    dm: &DecorMap,
    game: &Game,
    rng: &mut impl Rng,
) -> Task {
    if npc.stay <= 0.0 {
        return Task::Leave;
    }
    let who = npc.who;
    let mine: Vec<(IVec2, &farming::Plot)> = zone_tiles(isl, who)
        .into_iter()
        .filter_map(|p| farm.get(p).map(|pl| (p, pl)))
        .collect();
    if let Some((p, _)) = mine.iter().find(|(_, pl)| pl.ready()) {
        return Task::Harvest(*p);
    }
    if let Some((p, _)) = mine.iter().find(|(_, pl)| pl.crop.is_some() && !pl.watered) {
        return Task::Water(*p);
    }
    if let Some((p, _)) = mine.iter().find(|(_, pl)| pl.crop.is_none()) {
        let crops: Vec<Crop> = Crop::ALL.iter().copied().filter(|c| game.crop_unlocked(*c)).collect();
        let pref = match who {
            Who::Tyoma => Crop::Potato,
            Who::Mila => Crop::Cabbage,
            _ => Crop::Carrot,
        };
        let c = if game.crop_unlocked(pref) && rng.gen::<f32>() < 0.5 { pref } else { *crops.choose(rng).unwrap() };
        return Task::Plant(*p, c);
    }
    let want = match who {
        Who::Mila => 8,
        Who::Tyoma => 5,
        _ => 4,
    };
    if mine.len() < want {
        if let Some(p) = next_bed_tile(isl, farm, dm, who) {
            return Task::Till(p);
        }
    }
    match who {
        Who::Mila => {
            // waters dry crops of others — yours first
            let mut dry: Vec<(IVec2, Who)> = farm
                .plots
                .iter()
                .filter(|(_, pl)| pl.crop.is_some() && !pl.watered && pl.planter != Who::Mila)
                .map(|(k, pl)| (IVec2::new(k.0, k.1), pl.planter))
                .collect();
            dry.sort_by_key(|(_, w)| (*w != Who::You) as u8);
            if !dry.is_empty() && rng.gen::<f32>() < 0.75 {
                let pick = dry[rng.gen_range(0..dry.len().min(4))].0;
                return Task::Water(pick);
            }
        }
        Who::Tyoma => {
            if npc.cook_cd <= 0.0 {
                // only cooks from surplus so he never empties the shared pantry
                let mut options: Vec<Dish> = Dish::ALL
                    .iter()
                    .copied()
                    .filter(|d| game.dish_unlocked(*d))
                    .filter(|d| d.recipe().iter().all(|(c, n)| game.count(Item::Crop(*c)) >= n + 5))
                    .collect();
                options.shuffle(rng);
                if let Some(d) = options.first() {
                    return Task::Cook(*d);
                }
            }
        }
        Who::Ksyu => {
            if npc.gift_cd <= 0.0 {
                let opts: Vec<Decor> = Decor::ALL
                    .iter()
                    .copied()
                    .filter(|d| d.unlock_level() <= game.level && d.season().is_none())
                    .collect();
                if let Some(d) = opts.choose(rng) {
                    return Task::Gift(*d);
                }
            }
            if npc.decor_cd <= 0.0 {
                let placed = dm.items.values().filter(|(_, w)| *w == Who::Ksyu).count();
                if placed < 7 {
                    let z = isl.zone(Owner::Ksyu);
                    for _ in 0..30 {
                        let p = IVec2::new(rng.gen_range(z.min.x..=z.max.x), rng.gen_range(z.min.y..=z.max.y));
                        let edge = p.x == z.min.x || p.x == z.max.x || p.y == z.min.y || p.y == z.max.y;
                        if (edge || rng.gen::<f32>() < 0.2) && isl.decor_ok(p) && farm.get(p).is_none() {
                            let d = *[Decor::Flowerbed, Decor::Fence, Decor::Lantern, Decor::Mushroom, Decor::Birdhouse]
                                .choose(rng)
                                .unwrap();
                            return Task::PlaceDecor(p, d);
                        }
                    }
                }
            }
        }
        Who::You => {}
    }
    Task::Wander
}

#[allow(clippy::too_many_arguments)]
fn npc_brain(
    time: Res<Time>,
    speed: Res<GameSpeed>,
    paused: Res<Paused>,
    mut commands: Commands,
    art: Res<Art>,
    mut island: ResMut<Island>,
    mut farm: ResMut<Farm>,
    mut dm: ResMut<DecorMap>,
    mut game: ResMut<Game>,
    mut q: Query<(Entity, &mut Npc, &mut Transform, &mut Visibility, &mut CharAnim, &mut Mover)>,
    mut fx: EventWriter<Fx>,
    mut notes: EventWriter<Note>,
    mut say: EventWriter<Say>,
    mut play: EventWriter<Play>,
) {
    if paused.0 {
        return;
    }
    let dt = time.delta_secs() * speed.0;
    let mut rng = rand::thread_rng();
    for (_e, mut npc, mut tr, mut vis, mut anim, mut mover) in &mut q {
        npc.stay -= dt;
        npc.cook_cd -= dt;
        npc.decor_cd -= dt;
        npc.gift_cd -= dt;
        npc.feed_cd -= dt;
        npc.chat_t -= dt;
        match npc.phase {
            Phase::Away => {
                if npc.stay <= 0.0 {
                    // arrive by boat
                    let dock = *island.dock.last().unwrap();
                    let p = tile_center(dock);
                    tr.translation = Vec3::new(p.x, p.y, ysort(p.y));
                    *vis = Visibility::Inherited;
                    npc.phase = Phase::Idle;
                    npc.wait = 1.6;
                    npc.stay = rng.gen_range(200.0..340.0);
                    commands.spawn((
                        Sprite::from_image(art.boat.clone()),
                        Transform::from_xyz(p.x + 22.0, p.y - 40.0, ysort(p.y) - 0.01),
                        Boat { t: 0.0, who: npc.who, leaving: false },
                    ));
                    play.write(Play::ui(S::Horn).vol(0.35));
                    notes.write(Note::Toast(
                        format!("{} заглядывает на остров ({})", npc.who.name(), npc.who.role()),
                        icon::HEART,
                    ));
                }
            }
            Phase::Idle => {
                mover.vel = Vec2::ZERO;
                npc.wait -= dt;
                if npc.wait > 0.0 {
                    continue;
                }
                let task = choose_task(&mut npc, &island, &farm, &dm, &game, &mut rng);
                let from = world_to_tile(tr.translation.truncate() + Vec2::Y * 3.0);
                match task_target(&island, &task, npc.who, &mut rng) {
                    Some(to) => match island.find_path(from, to, 8000) {
                        Some(path) => {
                            npc.path = path;
                            npc.task = Some(task);
                            npc.phase = Phase::Walking;
                            npc.stuck = 0.0;
                        }
                        None => {
                            npc.wait = 1.0;
                        }
                    },
                    None => npc.wait = 1.0,
                }
            }
            Phase::Walking => {
                // movement handled in npc_move; arrival switches to Working
                if npc.path.is_empty() {
                    npc.phase = Phase::Working;
                    npc.work_t = 0.45;
                    mover.vel = Vec2::ZERO;
                    if let Some(t) = &npc.task {
                        let (face, tool) = match t {
                            Task::Till(p) => (Some(*p), Some(icon::HOE)),
                            Task::Plant(p, c) => (Some(*p), Some(c.seed_icon())),
                            Task::Water(p) => (Some(*p), Some(icon::CAN)),
                            Task::Harvest(p) => (Some(*p), None),
                            Task::PlaceDecor(p, _) => (Some(*p), None),
                            _ => (None, None),
                        };
                        if let Some(fp) = face {
                            let d = tile_center(fp) - tr.translation.truncate();
                            anim.facing = crate::player::Facing::from_vel(d, anim.facing);
                            anim.action_t = 0.3;
                            anim.tool_icon = tool;
                        }
                    }
                }
            }
            Phase::Working => {
                npc.work_t -= dt;
                if npc.work_t > 0.0 {
                    continue;
                }
                let who = npc.who;
                let task = npc.task.take();
                npc.phase = Phase::Idle;
                npc.wait = rng.gen_range(0.6..2.2);
                match task {
                    Some(Task::Till(p)) => {
                        if farming::till(&mut farm, &island, &dm, who, p).is_ok() {
                            fx.write(Fx { kind: FxKind::Till, tile: p, who });
                        }
                    }
                    Some(Task::Plant(p, c)) => {
                        if farming::plant(&mut farm, &island, &mut game, who, p, c).is_ok() {
                            fx.write(Fx { kind: FxKind::Plant(c), tile: p, who });
                        }
                    }
                    Some(Task::Water(p)) => {
                        if let Ok(owner) = farming::water(&mut farm, p) {
                            fx.write(Fx { kind: FxKind::Water, tile: p, who });
                            game.stats.watered += 1;
                            game.add_xp(1, &mut notes);
                            let crop = farm.get(p).and_then(|pl| pl.crop);
                            if owner == Some(Who::You) {
                                farming::feed_for_water(who, owner, crop, &mut notes);
                                if rng.gen::<f32>() < 0.4 {
                                    say.write(Say(who, "Полила твои грядки!".into()));
                                }
                            }
                        }
                    }
                    Some(Task::Harvest(p)) => {
                        if let Ok(r) = farming::harvest(&mut farm, &island, &mut game, who, p, &mut rng) {
                            fx.write(Fx { kind: FxKind::Harvest(r.crop, r.amount), tile: p, who });
                            game.add_xp(2, &mut notes);
                            if npc.feed_cd <= 0.0 {
                                npc.feed_cd = 25.0;
                                notes.write(Note::Feed(format!(
                                    "{} собирает урожай — {} уходит на общий склад",
                                    who.name(),
                                    r.crop.name().to_lowercase()
                                )));
                            }
                        }
                    }
                    Some(Task::Cook(d)) => {
                        if d.recipe().iter().all(|(c, n)| game.count(Item::Crop(*c)) >= *n) {
                            for (c, n) in d.recipe() {
                                game.take(Item::Crop(*c), *n);
                            }
                            game.add(Item::Dish(d), 1);
                            *game.stats.cooked.entry(d).or_insert(0) += 1;
                            game.stats.cooked_total += 1;
                            game.add_xp(d.xp() / 2, &mut notes);
                            notes.write(Note::Toast(
                                format!("Тёма приготовил «{}» из излишков урожая", d.name()),
                                d.icon(),
                            ));
                            play.write(Play::at(S::Cook, tr.translation.truncate()).vol(0.5));
                            say.write(Say(who, format!("{} готов! Угощайтесь", d.name())));
                            commands.spawn(crate::ui::ChimneySmoke(3.0));
                        }
                        npc.cook_cd = rng.gen_range(70.0..120.0);
                    }
                    Some(Task::PlaceDecor(p, d)) => {
                        if farming::place_decor(&mut dm, &farm, &mut island, &mut game, who, p, d).is_ok() {
                            fx.write(Fx { kind: FxKind::Place(d), tile: p, who });
                        }
                        npc.decor_cd = rng.gen_range(40.0..80.0);
                    }
                    Some(Task::Gift(d)) => {
                        game.add(Item::Decor(d), 1);
                        notes.write(Note::Toast(format!("Ксю подарила острову: {}", d.name()), icon::HEART));
                        say.write(Say(who, "Положила подарок в общий сундук!".into()));
                        play.write(Play::at(S::Quest, tr.translation.truncate()).vol(0.4));
                        npc.gift_cd = rng.gen_range(160.0..260.0);
                    }
                    Some(Task::Leave) => {
                        *vis = Visibility::Hidden;
                        let p = tr.translation;
                        commands.spawn((
                            Sprite::from_image(art.boat.clone()),
                            Transform::from_xyz(p.x + 22.0, p.y - 10.0, p.z - 0.01),
                            Boat { t: 0.0, who, leaving: true },
                        ));
                        tr.translation = Vec3::new(-5000.0, 5000.0, 0.0);
                        npc.phase = Phase::Away;
                        npc.stay = rng.gen_range(60.0..140.0);
                        play.write(Play::ui(S::Horn).vol(0.25));
                        notes.write(Note::Feed(format!("{} уплывает отдохнуть — вернётся позже", who.name())));
                    }
                    Some(Task::Wander) | None => {
                        npc.wait = rng.gen_range(1.5..4.0);
                    }
                }
            }
        }
    }
}

fn npc_move(
    time: Res<Time>,
    speed: Res<GameSpeed>,
    island: Res<Island>,
    mut q: Query<(&mut Npc, &mut Transform, &mut Mover)>,
) {
    let dt = time.delta_secs().min(0.05) * speed.0.min(4.0);
    for (mut npc, mut tr, mut mover) in &mut q {
        if npc.phase != Phase::Walking {
            continue;
        }
        let Some(next) = npc.path.first().copied() else { continue };
        let goal = tile_center(next) - Vec2::Y * 4.0;
        let pos = tr.translation.truncate();
        let d = goal - pos;
        let spd = 46.0;
        if d.length() < 2.0 {
            npc.path.remove(0);
            if npc.path.is_empty() {
                mover.vel = Vec2::ZERO;
            }
            continue;
        }
        if !island.walkable(next) {
            // something got placed on the way: give up, rethink
            npc.path.clear();
            npc.task = None;
            npc.phase = Phase::Idle;
            npc.wait = 0.5;
            continue;
        }
        let want = d.normalize() * spd;
        mover.vel = mover.vel.lerp(want, (dt * 10.0).min(1.0));
        let np = move_with_collision(&island, pos, mover.vel * dt);
        if np.distance(pos) < 0.01 {
            npc.stuck += dt;
            // corners: nudge straight to the tile center
            let np2 = pos + d.clamp_length_max(spd * dt);
            tr.translation.x = np2.x;
            tr.translation.y = np2.y;
        } else {
            tr.translation.x = np.x;
            tr.translation.y = np.y;
        }
        tr.translation.z = ysort(tr.translation.y);
    }
}

fn boats(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Boat, &mut Transform, &mut Sprite)>) {
    for (e, mut b, mut tr, mut s) in &mut q {
        b.t += time.delta_secs();
        let bob = (b.t * 3.0).sin() * 0.8;
        if b.leaving {
            tr.translation.y -= 30.0 * time.delta_secs();
            s.color.set_alpha((1.0 - b.t / 2.5).max(0.0));
            if b.t > 2.5 {
                commands.entity(e).despawn();
            }
        } else {
            let k = (b.t / 1.2).min(1.0);
            tr.translation.y += (30.0 * (1.0 - k)) * time.delta_secs() + bob * 0.02;
            if b.t > 3.0 {
                s.color.set_alpha((1.0 - (b.t - 3.0) / 1.5).max(0.0));
                tr.translation.y -= 20.0 * time.delta_secs();
            }
            if b.t > 4.5 {
                commands.entity(e).despawn();
            }
        }
        let _ = b.who;
    }
}

const LINES: &[(Who, &str)] = &[
    (Who::Mila, "Если что, я полью твои грядки"),
    (Who::Mila, "Капуста любит весну и осень"),
    (Who::Mila, "Полив ускоряет рост вдвое!"),
    (Who::Mila, "Ничего не завянет, не переживай"),
    (Who::Mila, "Как же тут хорошо пахнет землёй"),
    (Who::Tyoma, "Я готовлю только из излишков, честно"),
    (Who::Tyoma, "Блюда дороже сырых овощей"),
    (Who::Tyoma, "Зимой готовить особенно уютно"),
    (Who::Tyoma, "Морковный пирог — моя слабость"),
    (Who::Tyoma, "Кухня общая — заходи, готовь"),
    (Who::Ksyu, "Уют приманивает питомцев"),
    (Who::Ksyu, "Фонари красиво светят ночью"),
    (Who::Ksyu, "Сезонный декор вернётся через год"),
    (Who::Ksyu, "Моя зона — моё творчество!"),
    (Who::Ksyu, "Миска для питомцев — хорошая идея"),
];

fn talk(
    mut ev: EventReader<Interact>,
    mut say: EventWriter<Say>,
    mut play: EventWriter<Play>,
    npcs: Query<(&Npc, &Transform)>,
    mut commands: Commands,
) {
    for i in ev.read() {
        if let Interact::Npc(who) = i {
            let mut rng = rand::thread_rng();
            let lines: Vec<&str> = LINES.iter().filter(|(w, _)| w == who).map(|(_, l)| *l).collect();
            let line = lines.choose(&mut rng).copied().unwrap_or("Привет!");
            let greet = ["Привет!", "О, привет!", "Хорошего дня!"].choose(&mut rng).unwrap();
            say.write(Say(*who, format!("{} {}", greet, line)));
            play.write(Play::ui(S::Select).vol(0.4).pitch(1.3));
            if let Some((_, t)) = npcs.iter().find(|(n, _)| n.who == *who) {
                let p = t.translation.truncate() + Vec2::Y * 24.0;
                for k in 0..3 {
                    let e = crate::juice::spawn_particle(
                        &mut commands,
                        p + Vec2::new(k as f32 * 4.0 - 4.0, 0.0),
                        55.0,
                        Vec2::new(rng.gen_range(-8.0..8.0), rng.gen_range(14.0..24.0)),
                        Color::srgb(1.0, 0.45, 0.55),
                        3.0,
                        0.9,
                        0.0,
                    );
                    let _ = e;
                }
            }
        }
    }
}

fn bubbles(
    mut commands: Commands,
    art: Res<Art>,
    time: Res<Time>,
    mut ev: EventReader<Say>,
    actors: Res<Actors>,
    mut q: Query<(Entity, &mut Bubble, &mut WorldAnchor)>,
    mut existing: Local<Vec<(Who, Entity)>>,
) {
    for Say(who, text) in ev.read() {
        // one bubble per speaker
        existing.retain(|(w, e)| {
            if w == who {
                commands.entity(*e).despawn();
                false
            } else {
                true
            }
        });
        let Some(&target) = actors.0.get(who) else { continue };
        let c = who.color();
        let e = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                    max_width: Val::Px(220.0),
                    ..default()
                },
                ImageNode {
                    image: art.ui_bubble.clone(),
                    image_mode: NodeImageMode::Sliced(TextureSlicer {
                        border: BorderRect::all(5.0),
                        ..default()
                    }),
                    ..default()
                },
                WorldAnchor { pos: Vec2::ZERO, follow: Some(target), offset: Vec2::new(0.0, 30.0) },
                Bubble { t: 0.0 },
                GlobalZIndex(3),
                Pickable::IGNORE,
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(format!("{}: ", who.name())),
                    TextFont { font: art.font.clone(), font_size: 13.0, ..default() },
                    TextColor(Color::srgb(c[0] * 0.7, c[1] * 0.7, c[2] * 0.7)),
                    Pickable::IGNORE,
                ))
                .with_child((
                    TextSpan::new(text.clone()),
                    TextFont { font: art.font.clone(), font_size: 13.0, ..default() },
                    TextColor(Color::srgb(0.18, 0.13, 0.22)),
                ));
            })
            .id();
        existing.push((*who, e));
    }
    for (e, mut b, _) in &mut q {
        b.t += time.delta_secs();
        if b.t > 4.2 {
            commands.entity(e).despawn();
            existing.retain(|(_, x)| *x != e);
        }
    }
    let _ = Anchor::Center;
}

fn chatter(
    mut q: Query<(&mut Npc, &Transform)>,
    player: Query<&Transform, (With<Player>, Without<Npc>)>,
    mut say: EventWriter<Say>,
) {
    let Ok(pt) = player.single() else { return };
    let mut rng = rand::thread_rng();
    for (mut npc, t) in &mut q {
        if npc.phase == Phase::Away || npc.chat_t > 0.0 {
            continue;
        }
        npc.chat_t = rng.gen_range(25.0..50.0);
        if t.translation.truncate().distance(pt.translation.truncate()) < 110.0 {
            let lines: Vec<&str> = LINES.iter().filter(|(w, _)| *w == npc.who).map(|(_, l)| *l).collect();
            if let Some(l) = lines.choose(&mut rng) {
                say.write(Say(npc.who, l.to_string()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::MAP_TXT;

    #[test]
    fn friends_have_room_for_beds_and_never_touch_your_zone() {
        let isl = Island::parse(MAP_TXT);
        let farm = Farm::default();
        let dm = DecorMap::default();
        for w in Who::FRIENDS {
            let p = next_bed_tile(&isl, &farm, &dm, w).expect("bed tile");
            assert_eq!(isl.owner(p), w.zone());
        }
    }
}
