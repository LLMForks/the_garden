//! The player: movement, animation, tools, targeting and interactions.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::PrimaryWindow;
use rand::Rng;

use crate::assets::Art;
use crate::data::*;
use crate::farming::{self, DecorMap, Deny, Farm, Fx, FxKind};
use crate::juice::{spawn_particle, Actors, Mover, Particle, Play, Shake, S};
use crate::state::{Game, Note};
use crate::ui::UiState;
use crate::world::{shadow, tile_center, world_to_tile, Building, Ground, Interactable, Island};
use crate::{ysort, MainCamera, Player, TILE};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Facing {
    #[default]
    Down,
    Up,
    Left,
    Right,
}

impl Facing {
    pub fn vec(self) -> IVec2 {
        match self {
            Facing::Down => IVec2::new(0, 1),
            Facing::Up => IVec2::new(0, -1),
            Facing::Left => IVec2::new(-1, 0),
            Facing::Right => IVec2::new(1, 0),
        }
    }
    pub fn from_vel(v: Vec2, cur: Facing) -> Facing {
        if v.length_squared() < 1.0 {
            return cur;
        }
        if v.x.abs() > v.y.abs() * 1.1 {
            if v.x > 0.0 { Facing::Right } else { Facing::Left }
        } else if v.y > 0.0 {
            Facing::Up
        } else {
            Facing::Down
        }
    }
}

/// Shared character animation (player and friends).
#[derive(Component, Default)]
pub struct CharAnim {
    pub facing: Facing,
    pub phase: f32,
    pub action_t: f32,
    pub last_frame: usize,
    pub tool_icon: Option<usize>,
    pub quiet: bool,
}

#[derive(Component)]
pub struct ToolSprite;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Hoe,
    Can,
    Seeds(Crop),
    Decor,
}

pub const SLOTS: [Slot; 7] = [
    Slot::Hoe,
    Slot::Can,
    Slot::Seeds(Crop::Wheat),
    Slot::Seeds(Crop::Carrot),
    Slot::Seeds(Crop::Cabbage),
    Slot::Seeds(Crop::Potato),
    Slot::Decor,
];

#[derive(Resource)]
pub struct Hotbar {
    pub sel: usize,
    pub decor: Decor,
    pub changed_t: f32,
}

impl Hotbar {
    pub fn slot(&self) -> Slot {
        SLOTS[self.sel]
    }
}

#[derive(Resource, Default)]
pub struct Target {
    pub tile: IVec2,
    pub plan: Planned,
    pub mouse_active: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Planned {
    #[default]
    None,
    Harvest,
    Till,
    Plant(Crop),
    Water,
    Refill,
    Place(Decor),
    Deny(String),
}

#[derive(Event, Clone, Copy, Debug)]
pub enum Interact {
    Building(Building),
    Sign(Owner),
    Npc(Who),
    Pet(Entity),
}

/// Things the player can walk up to and press E.
#[derive(Component)]
pub struct Talkable(pub Who);

#[derive(Component)]
pub struct Cursor;
#[derive(Component)]
pub struct Ghost;

#[derive(Resource, Default)]
struct HoldState {
    last_tile: Option<IVec2>,
    cooldown: f32,
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Hotbar { sel: 0, decor: Decor::Fence, changed_t: 0.0 })
            .init_resource::<Target>()
            .init_resource::<HoldState>()
            .add_event::<Interact>()
            .add_systems(Startup, spawn_player)
            .add_systems(
                Update,
                (
                    (hotbar_input, move_player, targeting, use_tools, interact_key).chain(),
                    animate_chars,
                    camera_follow,
                    draw_cursor,
                ),
            );
    }
}

pub fn spawn_character(commands: &mut Commands, art: &Art, who: Who, pos: Vec2) -> Entity {
    let e = commands
        .spawn((
            Sprite {
                anchor: Anchor::Custom(Vec2::new(0.0, -0.5 + 1.0 / 26.0)),
                ..Sprite::from_atlas_image(art.char(who), TextureAtlas { layout: art.char_layout.clone(), index: 0 })
            },
            Transform::from_xyz(pos.x, pos.y, ysort(pos.y)),
            CharAnim::default(),
            Mover::default(),
        ))
        .with_children(|p| {
            p.spawn((
                Sprite {
                    custom_size: Some(Vec2::new(12.0, 5.0)),
                    color: Color::srgba(1.0, 1.0, 1.0, 1.0),
                    ..Sprite::from_image(art.shadow.clone())
                },
                Transform::from_xyz(0.0, 1.0, -0.05),
            ));
            p.spawn((
                Sprite { anchor: Anchor::BottomCenter, ..art.icon(icon::HOE) },
                Transform::from_xyz(6.0, 6.0, 0.02),
                Visibility::Hidden,
                ToolSprite,
            ));
        })
        .id();
    e
}

fn spawn_player(mut commands: Commands, art: Res<Art>, island: Res<Island>, mut actors: ResMut<Actors>) {
    let z = island.zone(Owner::You);
    let start = IVec2::new(z.max.x - 1, z.min.y + 1);
    let pos = tile_center(start) - Vec2::Y * 4.0;
    let e = spawn_character(&mut commands, &art, Who::You, pos);
    commands.entity(e).insert(Player).with_child((
        Sprite {
            color: Color::srgba(1.0, 0.8, 0.5, 0.0),
            custom_size: Some(Vec2::splat(110.0)),
            ..Sprite::from_image(art.light.clone())
        },
        Transform::from_xyz(0.0, 10.0, 56.0),
        crate::farming::GlowLight { base: 0.14, phase: 0.0 },
    ));
    actors.0.insert(Who::You, e);
    commands.spawn((
        Sprite::from_image(art.cursor.clone()),
        Transform::from_xyz(pos.x, pos.y, 49.0),
        Cursor,
    ));
    commands.spawn((
        Sprite { anchor: Anchor::BottomCenter, color: Color::srgba(1.0, 1.0, 1.0, 0.5), ..Sprite::from_image(art.decor(Decor::Fence)) },
        Transform::from_xyz(pos.x, pos.y, 48.0),
        Visibility::Hidden,
        Ghost,
    ));
    let _ = shadow;
}

fn hotbar_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: EventReader<MouseWheel>,
    mut hb: ResMut<Hotbar>,
    game: Res<Game>,
    ui: Res<UiState>,
    time: Res<Time>,
    mut play: EventWriter<Play>,
) {
    hb.changed_t += time.delta_secs();
    if ui.blocking() {
        wheel.clear();
        return;
    }
    let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7];
    let old = hb.sel;
    for (i, k) in digits.iter().enumerate() {
        if keys.just_pressed(*k) {
            hb.sel = i;
        }
    }
    let mut dy = 0.0;
    for w in wheel.read() {
        dy += w.y;
    }
    if dy > 0.0 {
        hb.sel = (hb.sel + SLOTS.len() - 1) % SLOTS.len();
    } else if dy < 0.0 {
        hb.sel = (hb.sel + 1) % SLOTS.len();
    }
    if keys.just_pressed(KeyCode::KeyR) || keys.just_pressed(KeyCode::KeyX) {
        // cycle owned decor
        let owned: Vec<Decor> = Decor::ALL.iter().copied().filter(|d| game.count(Item::Decor(*d)) > 0).collect();
        if !owned.is_empty() {
            let i = owned.iter().position(|d| *d == hb.decor).map(|i| (i + 1) % owned.len()).unwrap_or(0);
            hb.decor = owned[i];
            hb.sel = 6;
            hb.changed_t = 0.0;
            play.write(Play::ui(S::Select));
        }
    }
    if game.count(Item::Decor(hb.decor)) == 0 {
        if let Some(d) = Decor::ALL.iter().copied().find(|d| game.count(Item::Decor(*d)) > 0) {
            hb.decor = d;
        }
    }
    if hb.sel != old {
        hb.changed_t = 0.0;
        play.write(Play::ui(S::Select).vol(0.4));
    }
}

fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    island: Res<Island>,
    ui: Res<UiState>,
    mut q: Query<(&mut Transform, &mut Mover, &mut CharAnim), With<Player>>,
) {
    let Ok((mut tr, mut mover, mut anim)) = q.single_mut() else { return };
    let dt = time.delta_secs().min(0.05);
    let mut dir = Vec2::ZERO;
    if !ui.blocking() {
        if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
            dir.y += 1.0;
        }
        if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
            dir.y -= 1.0;
        }
        if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
            dir.x -= 1.0;
        }
        if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
            dir.x += 1.0;
        }
    }
    let run = if keys.pressed(KeyCode::ShiftLeft) { 1.45 } else { 1.0 };
    let target = dir.normalize_or_zero() * 74.0 * run;
    let k = if target.length_squared() > 0.0 { 16.0 } else { 20.0 };
    mover.vel = mover.vel.lerp(target, 1.0 - (-dt * k).exp());
    if mover.vel.length_squared() < 0.5 {
        mover.vel = Vec2::ZERO;
    }
    let pos = tr.translation.truncate();
    let np = move_with_collision(&island, pos, mover.vel * dt);
    tr.translation.x = np.x;
    tr.translation.y = np.y;
    tr.translation.z = ysort(np.y);
    if dir != Vec2::ZERO {
        anim.facing = Facing::from_vel(dir, anim.facing);
    }
}

/// Moves a character's feet (small AABB) through the tile grid with sliding.
pub fn move_with_collision(island: &Island, pos: Vec2, delta: Vec2) -> Vec2 {
    let half = Vec2::new(4.5, 2.5);
    let blocked = |p: Vec2| {
        let corners = [
            Vec2::new(p.x - half.x, p.y - 0.5),
            Vec2::new(p.x + half.x, p.y - 0.5),
            Vec2::new(p.x - half.x, p.y + half.y * 2.0 - 0.5),
            Vec2::new(p.x + half.x, p.y + half.y * 2.0 - 0.5),
        ];
        corners.iter().any(|c| !island.walkable(world_to_tile(*c)))
    };
    let mut p = pos;
    if blocked(p) {
        return p + delta; // stuck inside something: let them walk out
    }
    let nx = Vec2::new(p.x + delta.x, p.y);
    if !blocked(nx) {
        p = nx;
    }
    let ny = Vec2::new(p.x, p.y + delta.y);
    if !blocked(ny) {
        p = ny;
    }
    p
}

pub fn feet_tile(tr: &Transform) -> IVec2 {
    world_to_tile(tr.translation.truncate() + Vec2::Y * 3.0)
}

fn plan_action(
    tile: IVec2,
    slot: Slot,
    decor_sel: Decor,
    farm: &Farm,
    dm: &DecorMap,
    isl: &Island,
    game: &Game,
) -> Planned {
    let owner = isl.owner(tile);
    if let Some(p) = farm.get(tile) {
        if p.ready() {
            return if owner.can_build(Who::You) { Planned::Harvest } else { Planned::Deny(Deny::NotYours(owner).text()) };
        }
    }
    match slot {
        Slot::Hoe => {
            if farm.get(tile).is_some() {
                Planned::None
            } else if dm.items.contains_key(&(tile.x, tile.y)) {
                Planned::Deny(Deny::Occupied.text())
            } else if !isl.farmable(tile) {
                Planned::Deny(Deny::CantTill.text())
            } else if !owner.can_build(Who::You) {
                Planned::Deny(Deny::NotYours(owner).text())
            } else {
                Planned::Till
            }
        }
        Slot::Can => {
            if isl.ground_at(tile) == Ground::Water {
                return Planned::Refill;
            }
            match farm.get(tile) {
                Some(p) if !p.watered => {
                    if game.can_water == 0 {
                        Planned::Deny(Deny::NoWater.text())
                    } else {
                        Planned::Water
                    }
                }
                Some(_) => Planned::None,
                None => Planned::None,
            }
        }
        Slot::Seeds(c) => match farm.get(tile) {
            Some(p) if p.crop.is_none() => {
                if !owner.can_build(Who::You) {
                    Planned::Deny(Deny::NotYours(owner).text())
                } else if !game.crop_unlocked(c) {
                    Planned::Deny(Deny::Locked(c.unlock_level()).text())
                } else if game.count(Item::Seeds(c)) == 0 {
                    Planned::Deny(Deny::NoSeeds(c).text())
                } else {
                    Planned::Plant(c)
                }
            }
            Some(_) => Planned::None,
            None => {
                if isl.farmable(tile) {
                    Planned::Deny("Сначала вскопай землю мотыгой (1)".into())
                } else {
                    Planned::None
                }
            }
        },
        Slot::Decor => {
            if game.count(Item::Decor(decor_sel)) == 0 {
                return Planned::Deny("Нет украшений — купи в лавке".into());
            }
            if farm.get(tile).is_some() || dm.items.contains_key(&(tile.x, tile.y)) || !isl.decor_ok(tile) {
                Planned::Deny(Deny::Occupied.text())
            } else if !owner.can_build(Who::You) {
                Planned::Deny(Deny::NotYours(owner).text())
            } else {
                Planned::Place(decor_sel)
            }
        }
    }
}

fn targeting(
    windows: Query<&Window, With<PrimaryWindow>>,
    cam: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    player: Query<(&Transform, &CharAnim), With<Player>>,
    hb: Res<Hotbar>,
    farm: Res<Farm>,
    dm: Res<DecorMap>,
    island: Res<Island>,
    game: Res<Game>,
    mut target: ResMut<Target>,
    mut last_mouse: Local<(Vec2, f32)>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    no_mouse: Option<Res<crate::autotest::NoMouse>>,
) {
    let Ok((ptr, anim)) = player.single() else { return };
    let ptile = feet_tile(ptr);
    let mut tile = ptile + anim.facing.vec();
    let now = time.elapsed_secs();
    let mut mouse_ok = false;
    if let (Ok(w), Ok((camera, ct)), None) = (windows.single(), cam.single(), no_mouse) {
        if let Some(cp) = w.cursor_position() {
            if cp.distance(last_mouse.0) > 2.0 {
                *last_mouse = (cp, now);
            }
            let moving_keys = keys.any_pressed([KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD]);
            if now - last_mouse.1 < 3.0 || !moving_keys {
                if let Ok(wp) = camera.viewport_to_world_2d(ct, cp) {
                    let mt = world_to_tile(wp);
                    let d = mt - ptile;
                    if d.x.abs() <= 1 && d.y.abs() <= 1 && (d.x != 0 || d.y != 0 || hb.slot() != Slot::Decor) {
                        tile = mt;
                        mouse_ok = true;
                    } else if d.x.abs() <= 1 && d.y.abs() <= 1 {
                        tile = mt;
                        mouse_ok = true;
                    }
                }
            }
        }
    }
    target.tile = tile;
    target.mouse_active = mouse_ok;
    target.plan = plan_action(tile, hb.slot(), hb.decor, &farm, &dm, &island, &game);
}

#[allow(clippy::too_many_arguments)]
fn use_tools(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    ui: Res<UiState>,
    target: Res<Target>,
    mut hold: ResMut<HoldState>,
    mut farm: ResMut<Farm>,
    mut dm: ResMut<DecorMap>,
    mut island: ResMut<Island>,
    mut game: ResMut<Game>,
    mut fx: EventWriter<Fx>,
    mut notes: EventWriter<Note>,
    mut play: EventWriter<Play>,
    mut player: Query<(&Transform, &mut CharAnim), With<Player>>,
    hovered_ui: Res<crate::ui::PointerOverUi>,
) {
    hold.cooldown -= time.delta_secs();
    let Ok((ptr, mut anim)) = player.single_mut() else { return };
    if ui.blocking() {
        hold.last_tile = None;
        return;
    }
    let pressed_now = (mouse.just_pressed(MouseButton::Left) && !hovered_ui.0) || keys.just_pressed(KeyCode::Space);
    let held = (mouse.pressed(MouseButton::Left) && !hovered_ui.0) || keys.pressed(KeyCode::Space);
    // right click: pick up decor
    if mouse.just_pressed(MouseButton::Right) && !hovered_ui.0 {
        match farming::pickup_decor(&mut dm, &mut island, &mut game, Who::You, target.tile) {
            Ok(d) => {
                fx.write(Fx { kind: FxKind::Pickup, tile: target.tile, who: Who::You });
                notes.write(Note::Feed(format!("{} — обратно на склад", d.name())));
                anim.action_t = 0.2;
            }
            Err(Deny::Nothing) => {}
            Err(e) => {
                fx.write(Fx { kind: FxKind::Deny(e.text()), tile: target.tile, who: Who::You });
            }
        }
    }
    if !held {
        hold.last_tile = None;
        return;
    }
    if !pressed_now && (hold.cooldown > 0.0 || hold.last_tile == Some(target.tile)) {
        return;
    }
    if !pressed_now && matches!(target.plan, Planned::Deny(_) | Planned::None) {
        return;
    }
    if pressed_now && hold.cooldown > 0.08 {
        return;
    }
    hold.last_tile = Some(target.tile);
    hold.cooldown = 0.2;
    let t = target.tile;
    let face_to = tile_center(t) - ptr.translation.truncate();
    if face_to.length() > 4.0 {
        anim.facing = Facing::from_vel(face_to, anim.facing);
    }
    let mut rng = rand::thread_rng();
    match target.plan.clone() {
        Planned::Harvest => {
            if let Ok(r) = farming::harvest(&mut farm, &island, &mut game, Who::You, t, &mut rng) {
                fx.write(Fx { kind: FxKind::Harvest(r.crop, r.amount), tile: t, who: Who::You });
                game.add_xp(2, &mut notes);
                game.add_skill_xp(Skill::Gardener, 3, &mut notes);
                if r.amount > 1 {
                    notes.write(Note::Feed(format!("Удача садовника: двойной урожай! ({})", r.crop.name())));
                }
                if game.tutorial == 3 {
                    game.tutorial = 4;
                }
                anim.action_t = 0.22;
                anim.tool_icon = None;
            }
        }
        Planned::Till => {
            if farming::till(&mut farm, &island, &dm, Who::You, t).is_ok() {
                fx.write(Fx { kind: FxKind::Till, tile: t, who: Who::You });
                game.add_skill_xp(Skill::Gardener, 1, &mut notes);
                if game.tutorial == 0 {
                    game.tutorial = 1;
                }
                anim.action_t = 0.25;
                anim.tool_icon = Some(icon::HOE);
            }
        }
        Planned::Plant(c) => {
            if farming::plant(&mut farm, &island, &mut game, Who::You, t, c).is_ok() {
                fx.write(Fx { kind: FxKind::Plant(c), tile: t, who: Who::You });
                game.add_skill_xp(Skill::Gardener, 1, &mut notes);
                if game.tutorial == 1 {
                    game.tutorial = 2;
                }
                anim.action_t = 0.2;
                anim.tool_icon = Some(c.seed_icon());
            }
        }
        Planned::Water => {
            if let Ok(owner) = farming::water(&mut farm, t) {
                game.can_water -= 1;
                game.stats.watered += 1;
                fx.write(Fx { kind: FxKind::Water, tile: t, who: Who::You });
                let crop = farm.get(t).and_then(|p| p.crop);
                farming::feed_for_water(Who::You, owner, crop, &mut notes);
                game.add_xp(1, &mut notes);
                game.add_skill_xp(Skill::Gardener, 1, &mut notes);
                if game.tutorial == 2 {
                    game.tutorial = 3;
                }
                anim.action_t = 0.3;
                anim.tool_icon = Some(icon::CAN);
            }
        }
        Planned::Refill => {
            game.can_water = CAN_CAPACITY;
            fx.write(Fx { kind: FxKind::Refill, tile: t, who: Who::You });
            anim.action_t = 0.3;
            anim.tool_icon = Some(icon::CAN);
        }
        Planned::Place(d) => {
            if farming::place_decor(&mut dm, &farm, &mut island, &mut game, Who::You, t, d).is_ok() {
                fx.write(Fx { kind: FxKind::Place(d), tile: t, who: Who::You });
                game.add_skill_xp(Skill::Decorator, 4 + d.cozy() * 2, &mut notes);
                game.add_xp(3, &mut notes);
                if game.tutorial == 5 {
                    game.tutorial = 6;
                }
                anim.action_t = 0.2;
                anim.tool_icon = None;
            }
        }
        Planned::Deny(text) => {
            if pressed_now {
                fx.write(Fx { kind: FxKind::Deny(text), tile: t, who: Who::You });
            }
        }
        Planned::None => {
            if pressed_now {
                play.write(Play::ui(S::Click).vol(0.2));
            }
        }
    }
}

fn interact_key(
    keys: Res<ButtonInput<KeyCode>>,
    ui: Res<UiState>,
    player: Query<&Transform, With<Player>>,
    things: Query<(Entity, &Transform, &Interactable), Without<Player>>,
    npcs: Query<(&Transform, &Talkable), Without<Player>>,
    pets: Query<(Entity, &Transform), (With<crate::pets::Pet>, Without<Player>)>,
    island: Res<Island>,
    mut ev: EventWriter<Interact>,
    mut game: ResMut<Game>,
    mut fx: EventWriter<Fx>,
) {
    if ui.blocking() || !(keys.just_pressed(KeyCode::KeyE) || keys.just_pressed(KeyCode::KeyF)) {
        return;
    }
    let Ok(ptr) = player.single() else { return };
    let pp = ptr.translation.truncate() + Vec2::Y * 4.0;
    // pets & friends first (they move around)
    let mut best: Option<(f32, Interact)> = None;
    for (e, t) in &pets {
        let d = t.translation.truncate().distance(pp);
        if d < 22.0 && best.as_ref().is_none_or(|b| d < b.0) {
            best = Some((d, Interact::Pet(e)));
        }
    }
    for (t, tk) in &npcs {
        let d = (t.translation.truncate() + Vec2::Y * 4.0).distance(pp);
        if d < 24.0 && best.as_ref().is_none_or(|b| d < b.0) {
            best = Some((d, Interact::Npc(tk.0)));
        }
    }
    for (_, _, it) in &things {
        let (min, size, inter) = match it {
            Interactable::Building(b) => {
                let (p, s) = island.building(*b);
                (p, s, Interact::Building(*b))
            }
            Interactable::Sign(o) => {
                let p = island.signs.iter().find(|(oo, _)| oo == o).unwrap().1;
                (p, IVec2::ONE, Interact::Sign(*o))
            }
        };
        // distance from point to the footprint rectangle
        let lo = Vec2::new(min.x as f32 * TILE, -((min.y + size.y) as f32 * TILE));
        let hi = Vec2::new((min.x + size.x) as f32 * TILE, -(min.y as f32 * TILE));
        let c = pp.clamp(lo, hi);
        let d = c.distance(pp);
        if d < 14.0 && best.as_ref().is_none_or(|b| d + 4.0 < b.0) {
            best = Some((d + 4.0, inter));
        }
    }
    // water nearby for the can
    if best.is_none() && island.near_water(world_to_tile(pp)) {
        game.can_water = CAN_CAPACITY;
        fx.write(Fx { kind: FxKind::Refill, tile: world_to_tile(pp), who: Who::You });
        return;
    }
    if let Some((_, i)) = best {
        ev.write(i);
    }
}

pub fn animate_chars(
    time: Res<Time>,
    art: Res<Art>,
    mut commands: Commands,
    mut q: Query<(&mut Sprite, &mut CharAnim, &Mover, &Transform, &Children, Has<Player>)>,
    mut tools: Query<(&mut Sprite, &mut Transform, &mut Visibility), (With<ToolSprite>, Without<CharAnim>)>,
    mut play: EventWriter<Play>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();
    for (mut s, mut a, m, tr, children, is_player) in &mut q {
        let speed = m.vel.length();
        let moving = speed > 8.0;
        if moving {
            a.facing = Facing::from_vel(m.vel, a.facing);
            a.phase += dt * (speed / 74.0) * 9.0;
        } else {
            a.phase = 0.0;
        }
        a.action_t = (a.action_t - dt).max(0.0);
        let row = match a.facing {
            Facing::Down => 0,
            Facing::Up => 1,
            _ => 2,
        };
        let frame = if a.action_t > 0.0 {
            4
        } else if moving {
            (a.phase as usize) % 4
        } else {
            0
        };
        if moving && frame != a.last_frame && (frame == 1 || frame == 3) {
            let feet = tr.translation.truncate();
            if is_player {
                play.write(Play::ui(S::Step).vol(0.22));
            } else if !a.quiet {
                play.write(Play::at(S::Step, feet).vol(0.12));
            }
            // dust puff
            for _ in 0..2 {
                let e = spawn_particle(
                    &mut commands,
                    feet + Vec2::new(rng.gen_range(-3.0..3.0), 1.0),
                    tr.translation.z - 0.01,
                    Vec2::new(-m.vel.x * 0.15 + rng.gen_range(-6.0..6.0), rng.gen_range(4.0..10.0)),
                    Color::srgba(0.92, 0.88, 0.78, 0.6),
                    rng.gen_range(1.5..2.5),
                    0.35,
                    0.0,
                );
                commands.entity(e).entry::<Particle>().and_modify(|mut p| p.drag = 3.0);
            }
        }
        a.last_frame = frame;
        if let Some(at) = s.texture_atlas.as_mut() {
            at.index = row * 5 + frame;
        }
        s.flip_x = a.facing == Facing::Left;
        // tool swing
        for c in children.iter() {
            if let Ok((mut ts, mut tt, mut tv)) = tools.get_mut(c) {
                match (a.tool_icon, a.action_t > 0.0) {
                    (Some(idx), true) => {
                        *tv = Visibility::Inherited;
                        if let Some(at) = ts.texture_atlas.as_mut() {
                            at.index = idx;
                        }
                        let k = 1.0 - a.action_t / 0.3;
                        let (x, y, flip, z) = match a.facing {
                            Facing::Right => (7.0, 5.0, false, 0.02),
                            Facing::Left => (-7.0, 5.0, true, 0.02),
                            Facing::Down => (6.0, 3.0, false, 0.02),
                            Facing::Up => (-6.0, 8.0, true, -0.02),
                        };
                        ts.flip_x = flip;
                        let sign = if flip { -1.0 } else { 1.0 };
                        let ang = sign * (0.9 - k * 1.8);
                        tt.translation = Vec3::new(x, y, z);
                        tt.rotation = Quat::from_rotation_z(ang);
                    }
                    _ => *tv = Visibility::Hidden,
                }
            }
        }
    }
    let _ = art;
}

fn camera_follow(
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    player: Query<(&Transform, &Mover), (With<Player>, Without<MainCamera>)>,
    mut cam: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
    mut shake: ResMut<Shake>,
    island: Res<Island>,
    keys: Res<ButtonInput<KeyCode>>,
    mut zoom_adj: Local<i32>,
    mut pos: Local<Option<Vec2>>,
) {
    let Ok((ptr, mover)) = player.single() else { return };
    let Ok((mut ct, mut proj)) = cam.single_mut() else { return };
    let dt = time.delta_secs().min(0.05);
    if keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd) {
        *zoom_adj = (*zoom_adj + 1).min(3);
    }
    if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
        *zoom_adj = (*zoom_adj - 1).max(-2);
    }
    let h = windows.single().map(|w| w.height()).unwrap_or(720.0);
    let zoom = ((h / 250.0).round() as i32 + *zoom_adj).max(1) as f32;
    if let Projection::Orthographic(o) = proj.as_mut() {
        o.scale = 1.0 / zoom;
    }
    let goal = ptr.translation.truncate() + Vec2::Y * 12.0 + mover.vel * 0.22;
    let cur = pos.unwrap_or(goal);
    let mut next = cur.lerp(goal, 1.0 - (-dt * 6.0).exp());
    // keep a bit of ocean visible but don't fly off the map
    let w = island.w as f32 * TILE;
    let hh = island.h as f32 * TILE;
    next.x = next.x.clamp(0.0, w);
    next.y = next.y.clamp(-hh, 0.0);
    *pos = Some(next);
    shake.0 = (shake.0 - dt * 1.8).max(0.0);
    let tr = shake.0 * shake.0;
    let mut rng = rand::thread_rng();
    let off = Vec2::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0)) * tr * 6.0;
    let snap = |v: f32| (v * zoom).round() / zoom;
    ct.translation.x = snap(next.x + off.x);
    ct.translation.y = snap(next.y + off.y);
}

fn draw_cursor(
    time: Res<Time>,
    target: Res<Target>,
    art: Res<Art>,
    ui: Res<UiState>,
    mut q: Query<(&mut Transform, &mut Sprite, &mut Visibility), (With<Cursor>, Without<Ghost>)>,
    mut ghost: Query<(&mut Transform, &mut Sprite, &mut Visibility), (With<Ghost>, Without<Cursor>)>,
) {
    let Ok((mut tr, mut s, mut v)) = q.single_mut() else { return };
    let c = tile_center(target.tile);
    let goal = c.extend(49.0);
    tr.translation = tr.translation.lerp(goal, (time.delta_secs() * 25.0).min(1.0));
    if tr.translation.distance(goal) > 40.0 {
        tr.translation = goal;
    }
    let pulse = 1.0 + (time.elapsed_secs() * 6.0).sin() * 0.06;
    tr.scale = Vec3::splat(pulse);
    let col = match target.plan {
        Planned::None => Color::srgba(1.0, 1.0, 1.0, 0.35),
        Planned::Deny(_) => Color::srgba(1.0, 0.45, 0.4, 0.8),
        Planned::Harvest => Color::srgba(1.0, 0.9, 0.35, 1.0),
        _ => Color::srgba(0.7, 1.0, 0.6, 0.95),
    };
    s.color = col;
    *v = if ui.blocking() { Visibility::Hidden } else { Visibility::Inherited };
    if let Ok((mut gt, mut gs, mut gv)) = ghost.single_mut() {
        if let (Planned::Place(d), false) = (&target.plan, ui.blocking()) {
            *gv = Visibility::Inherited;
            gs.image = art.decor(*d);
            gs.color = Color::srgba(1.0, 1.0, 1.0, 0.45 + 0.1 * (time.elapsed_secs() * 5.0).sin());
            let b = crate::world::tile_bottom(target.tile);
            gt.translation = Vec3::new(b.x, b.y + 1.0, 48.0);
        } else {
            *gv = Visibility::Hidden;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::MAP_TXT;

    #[test]
    fn collision_blocks_water_and_buildings() {
        let isl = Island::parse(MAP_TXT);
        // standing in the middle of the shared zone, try walking into the kitchen
        let (kp, ks) = isl.building(Building::Kitchen);
        let front = isl.building_front(Building::Kitchen);
        let start = tile_center(front);
        let mut p = start;
        for _ in 0..100 {
            p = move_with_collision(&isl, p, Vec2::new(0.0, 2.0));
        }
        let t = world_to_tile(p + Vec2::Y * 1.0);
        assert!(t.y >= kp.y + ks.y, "walked into the kitchen: {:?}", t);
    }

    #[test]
    fn plan_respects_zones() {
        let isl = Island::parse(MAP_TXT);
        let farm = Farm::default();
        let dm = DecorMap::default();
        let game = Game::default();
        let z = isl.zone(Owner::Mila);
        let p = (z.min + z.max) / 2;
        let mut q = p;
        'outer: for y in z.min.y..=z.max.y {
            for x in z.min.x..=z.max.x {
                if isl.farmable(IVec2::new(x, y)) {
                    q = IVec2::new(x, y);
                    break 'outer;
                }
            }
        }
        assert!(matches!(plan_action(q, Slot::Hoe, Decor::Fence, &farm, &dm, &isl, &game), Planned::Deny(_)));
    }
}
