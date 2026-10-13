//! The island: tile map, zones, static objects, collisions and path finding.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use std::collections::{BinaryHeap, HashMap};

use crate::assets::{Art, MAP_TXT};
use crate::data::{Owner, Season, Who};
use crate::env::SeasonClock;
use crate::{ysort, TILE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    Water,
    Sand,
    Grass,
    Path,
    Dock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Building {
    Kitchen,
    Well,
    Shop,
    Chest,
    Crate,
    Board,
    Tent(Who),
}

#[derive(Clone, Debug)]
pub struct Zone {
    pub owner: Owner,
    pub min: IVec2,
    pub max: IVec2,
}

#[derive(Resource)]
pub struct Island {
    pub w: i32,
    pub h: i32,
    pub ground: Vec<Ground>,
    /// static obstacles (buildings, trees, rocks, signs)
    pub solid: Vec<bool>,
    /// dynamic obstacles (placed decor)
    pub decor_solid: Vec<bool>,
    pub zones: Vec<Zone>,
    pub buildings: Vec<(Building, IVec2, IVec2)>, // kind, top-left, size
    pub signs: Vec<(Owner, IVec2)>,
    pub nature: Vec<(String, IVec2)>,
    pub dock: Vec<IVec2>,
}

impl Island {
    pub fn parse(src: &str) -> Island {
        let mut w = 0;
        let mut h = 0;
        let mut rows: Vec<Vec<Ground>> = vec![];
        let mut zones = vec![];
        let mut buildings = vec![];
        let mut signs = vec![];
        let mut nature = vec![];
        for line in src.lines() {
            let mut parts = line.split_whitespace();
            match parts.next() {
                Some("SIZE") => {
                    w = parts.next().unwrap().parse().unwrap();
                    h = parts.next().unwrap().parse().unwrap();
                }
                Some("G") => {
                    let row = parts.next().unwrap_or("");
                    rows.push(
                        row.chars()
                            .map(|c| match c {
                                ':' => Ground::Sand,
                                ',' => Ground::Grass,
                                '=' => Ground::Path,
                                '#' => Ground::Dock,
                                _ => Ground::Water,
                            })
                            .collect(),
                    );
                }
                Some("ZONE") => {
                    let name = parts.next().unwrap();
                    let v: Vec<i32> = parts.map(|p| p.parse().unwrap()).collect();
                    zones.push(Zone {
                        owner: owner_from(name),
                        min: IVec2::new(v[0], v[1]),
                        max: IVec2::new(v[2], v[3]),
                    });
                }
                Some("OBJ") => {
                    let t = parts.next().unwrap();
                    let k = parts.next().unwrap().to_string();
                    let x: i32 = parts.next().unwrap().parse().unwrap();
                    let y: i32 = parts.next().unwrap().parse().unwrap();
                    match t {
                        "B" => {
                            let (b, size) = match k.as_str() {
                                "kitchen" => (Building::Kitchen, IVec2::new(4, 3)),
                                "well" => (Building::Well, IVec2::ONE),
                                "shop" => (Building::Shop, IVec2::new(2, 1)),
                                "chest" => (Building::Chest, IVec2::ONE),
                                "crate" => (Building::Crate, IVec2::ONE),
                                "board" => (Building::Board, IVec2::ONE),
                                "tent_you" => (Building::Tent(Who::You), IVec2::new(2, 2)),
                                "tent_mila" => (Building::Tent(Who::Mila), IVec2::new(2, 2)),
                                "tent_tyoma" => (Building::Tent(Who::Tyoma), IVec2::new(2, 2)),
                                _ => (Building::Tent(Who::Ksyu), IVec2::new(2, 2)),
                            };
                            buildings.push((b, IVec2::new(x, y), size));
                        }
                        "S" => signs.push((owner_from(&k), IVec2::new(x, y))),
                        _ => nature.push((k, IVec2::new(x, y))),
                    }
                }
                _ => {}
            }
        }
        let mut ground = vec![Ground::Water; (w * h) as usize];
        for (y, r) in rows.iter().enumerate() {
            for (x, g) in r.iter().enumerate() {
                ground[y * w as usize + x] = *g;
            }
        }
        let mut isl = Island {
            w,
            h,
            ground,
            solid: vec![false; (w * h) as usize],
            decor_solid: vec![false; (w * h) as usize],
            zones,
            buildings,
            signs,
            nature,
            dock: vec![],
        };
        for (_, p, s) in isl.buildings.clone() {
            for y in p.y..p.y + s.y {
                for x in p.x..p.x + s.x {
                    let i = isl.idx(IVec2::new(x, y));
                    isl.solid[i] = true;
                }
            }
        }
        for (_, p) in isl.signs.clone() {
            let i = isl.idx(p);
            isl.solid[i] = true;
        }
        for (_, p) in isl.nature.clone() {
            let i = isl.idx(p);
            isl.solid[i] = true;
        }
        for y in 0..h {
            for x in 0..w {
                if isl.ground_at(IVec2::new(x, y)) == Ground::Dock {
                    isl.dock.push(IVec2::new(x, y));
                }
            }
        }
        isl
    }

    pub fn in_bounds(&self, p: IVec2) -> bool {
        p.x >= 0 && p.y >= 0 && p.x < self.w && p.y < self.h
    }
    pub fn idx(&self, p: IVec2) -> usize {
        (p.y * self.w + p.x) as usize
    }
    pub fn ground_at(&self, p: IVec2) -> Ground {
        if !self.in_bounds(p) {
            return Ground::Water;
        }
        self.ground[self.idx(p)]
    }
    pub fn walkable(&self, p: IVec2) -> bool {
        self.in_bounds(p)
            && self.ground_at(p) != Ground::Water
            && !self.solid[self.idx(p)]
            && !self.decor_solid[self.idx(p)]
    }
    pub fn farmable(&self, p: IVec2) -> bool {
        self.in_bounds(p)
            && self.ground_at(p) == Ground::Grass
            && !self.solid[self.idx(p)]
            && !self.decor_solid[self.idx(p)]
    }
    pub fn decor_ok(&self, p: IVec2) -> bool {
        self.in_bounds(p)
            && matches!(self.ground_at(p), Ground::Grass | Ground::Sand)
            && !self.solid[self.idx(p)]
            && !self.decor_solid[self.idx(p)]
    }
    pub fn owner(&self, p: IVec2) -> Owner {
        for z in &self.zones {
            if z.owner != Owner::Shared && p.x >= z.min.x && p.y >= z.min.y && p.x <= z.max.x && p.y <= z.max.y
            {
                return z.owner;
            }
        }
        Owner::Shared
    }
    pub fn zone(&self, o: Owner) -> &Zone {
        self.zones.iter().find(|z| z.owner == o).unwrap()
    }
    pub fn building(&self, b: Building) -> (IVec2, IVec2) {
        let (_, p, s) = self.buildings.iter().find(|(k, _, _)| *k == b).unwrap();
        (*p, *s)
    }
    /// Tile right in front of a building (where you stand to use it).
    pub fn building_front(&self, b: Building) -> IVec2 {
        let (p, s) = self.building(b);
        let mut c = IVec2::new(p.x + s.x / 2, p.y + s.y);
        if b == Building::Shop {
            c.x = p.x;
        }
        c
    }
    pub fn near_water(&self, p: IVec2) -> bool {
        for d in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y, IVec2::ZERO] {
            let q = p + d;
            if self.ground_at(q) == Ground::Water {
                return true;
            }
        }
        false
    }

    /// A* on the tile grid (4-neighbour). Returns tiles excluding `from`.
    pub fn find_path(&self, from: IVec2, to: IVec2, max_nodes: usize) -> Option<Vec<IVec2>> {
        if from == to {
            return Some(vec![]);
        }
        let goal_ok = self.walkable(to);
        #[derive(PartialEq, Eq)]
        struct N(i32, IVec2);
        impl Ord for N {
            fn cmp(&self, o: &Self) -> std::cmp::Ordering {
                o.0.cmp(&self.0)
            }
        }
        impl PartialOrd for N {
            fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(o))
            }
        }
        let h = |p: IVec2| (p.x - to.x).abs() + (p.y - to.y).abs();
        let mut open = BinaryHeap::new();
        let mut came: HashMap<IVec2, IVec2> = HashMap::new();
        let mut cost: HashMap<IVec2, i32> = HashMap::new();
        open.push(N(h(from), from));
        cost.insert(from, 0);
        let mut n = 0;
        while let Some(N(_, cur)) = open.pop() {
            n += 1;
            if n > max_nodes {
                return None;
            }
            let adjacent_goal = !goal_ok && (cur - to).abs().element_sum() == 1;
            if cur == to || adjacent_goal {
                let mut path = vec![cur];
                let mut c = cur;
                while let Some(p) = came.get(&c) {
                    if *p == from {
                        break;
                    }
                    path.push(*p);
                    c = *p;
                }
                path.reverse();
                return Some(path);
            }
            for d in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                let nx = cur + d;
                if !self.walkable(nx) {
                    continue;
                }
                // paths are cheaper: NPCs prefer walking along them
                let step = match self.ground_at(nx) {
                    Ground::Path | Ground::Dock => 2,
                    _ => 3,
                };
                let nc = cost[&cur] + step;
                if cost.get(&nx).is_none_or(|c| nc < *c) {
                    cost.insert(nx, nc);
                    came.insert(nx, cur);
                    open.push(N(nc + h(nx) * 2, nx));
                }
            }
        }
        None
    }
}

fn owner_from(s: &str) -> Owner {
    match s {
        "you" => Owner::You,
        "mila" => Owner::Mila,
        "tyoma" => Owner::Tyoma,
        "ksyu" => Owner::Ksyu,
        _ => Owner::Shared,
    }
}

/// Tile -> world position of the tile center.
pub fn tile_center(p: IVec2) -> Vec2 {
    Vec2::new(p.x as f32 * TILE + TILE / 2.0, -(p.y as f32 * TILE + TILE / 2.0))
}
/// Tile -> world position of the tile's bottom-center (for sprites anchored at the feet).
pub fn tile_bottom(p: IVec2) -> Vec2 {
    Vec2::new(p.x as f32 * TILE + TILE / 2.0, -(p.y as f32 * TILE + TILE))
}
pub fn world_to_tile(v: Vec2) -> IVec2 {
    IVec2::new((v.x / TILE).floor() as i32, (-v.y / TILE).floor() as i32)
}

#[derive(Component)]
pub struct TerrainSprite;
#[derive(Component)]
pub struct FoamSprite;
#[derive(Component)]
pub struct WaterTile;
#[derive(Component)]
pub struct SeasonalTree(pub usize);
#[derive(Component)]
pub struct SeasonalBush;
#[derive(Component)]
pub struct KitchenSprite;
#[derive(Component)]
pub struct Chimney;

/// Something the player can interact with by pressing E nearby.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interactable {
    Building(Building),
    Sign(Owner),
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Island::parse(MAP_TXT))
            .add_systems(Startup, spawn_world)
            .add_systems(Update, (animate_water, apply_season_art));
    }
}

fn spawn_world(mut commands: Commands, art: Res<Art>, island: Res<Island>) {
    let pw = island.w as f32 * TILE;
    let ph = island.h as f32 * TILE;
    // ocean: animated 32x32 tiles, extended well beyond the island
    let margin = 30;
    for ty in (-margin..(island.h + margin) / 2 * 2 + margin).step_by(2) {
        for tx in (-margin..(island.w + margin) / 2 * 2 + margin).step_by(2) {
            let pos = Vec2::new(tx as f32 * TILE + TILE, -(ty as f32 * TILE + TILE));
            commands.spawn((
                Sprite::from_atlas_image(
                    art.water.clone(),
                    TextureAtlas { layout: art.water_layout.clone(), index: 0 },
                ),
                Transform::from_translation(pos.extend(-10.0)),
                WaterTile,
            ));
        }
    }
    commands.spawn((
        Sprite::from_image(art.foam[0].clone()),
        Transform::from_xyz(pw / 2.0, -ph / 2.0, -9.0),
        FoamSprite,
    ));
    commands.spawn((
        Sprite::from_image(art.terrain[0].clone()),
        Transform::from_xyz(pw / 2.0, -ph / 2.0, -8.0),
        TerrainSprite,
    ));
    // dock planks + boat post
    for p in &island.dock {
        let c = tile_center(*p);
        commands.spawn((Sprite::from_image(art.dock.clone()), Transform::from_xyz(c.x, c.y, -7.0)));
    }
    // dashed zone borders so you can see where personal zones are
    for z in island.zones.clone() {
        let c = match z.owner.who() {
            Some(w) => {
                let c = w.color();
                Color::srgba(c[0], c[1], c[2], 0.55)
            }
            None => Color::srgba(1.0, 0.95, 0.7, 0.35),
        };
        let x0 = z.min.x as f32 * TILE + 1.0;
        let x1 = (z.max.x + 1) as f32 * TILE - 1.0;
        let y0 = -(z.min.y as f32 * TILE) - 1.0;
        let y1 = -((z.max.y + 1) as f32 * TILE) + 1.0;
        let mut dash = |a: Vec2, size: Vec2| {
            commands.spawn((
                Sprite { color: c, custom_size: Some(size), ..default() },
                Transform::from_xyz(a.x, a.y, -6.5),
            ));
        };
        let mut x = x0;
        while x < x1 {
            dash(Vec2::new(x + 2.0, y0), Vec2::new(4.0, 2.0));
            dash(Vec2::new(x + 2.0, y1), Vec2::new(4.0, 2.0));
            x += 8.0;
        }
        let mut y = y1;
        while y < y0 {
            dash(Vec2::new(x0, y + 2.0), Vec2::new(2.0, 4.0));
            dash(Vec2::new(x1, y + 2.0), Vec2::new(2.0, 4.0));
            y += 8.0;
        }
    }
    // buildings
    for (b, p, s) in island.buildings.clone() {
        let foot = Vec2::new(
            p.x as f32 * TILE + s.x as f32 * TILE / 2.0,
            -((p.y + s.y) as f32 * TILE),
        );
        let (sprite, offset) = match b {
            Building::Kitchen => (
                Sprite::from_atlas_image(
                    art.kitchen.clone(),
                    TextureAtlas { layout: art.kitchen_layout.clone(), index: 0 },
                ),
                Vec2::new(0.0, -2.0),
            ),
            Building::Well => (Sprite::from_image(art.well.clone()), Vec2::new(0.0, -1.0)),
            Building::Shop => (Sprite::from_image(art.shop.clone()), Vec2::new(0.0, -1.0)),
            Building::Chest => (Sprite::from_image(art.chest.clone()), Vec2::ZERO),
            Building::Crate => (Sprite::from_image(art.crate_box.clone()), Vec2::ZERO),
            Building::Board => (Sprite::from_image(art.board.clone()), Vec2::new(0.0, -1.0)),
            Building::Tent(w) => (Sprite::from_image(art.tent(w)), Vec2::new(0.0, -1.0)),
        };
        let pos = foot + offset;
        let mut e = commands.spawn((
            Sprite { anchor: Anchor::BottomCenter, ..sprite },
            Transform::from_xyz(pos.x, pos.y, ysort(foot.y)),
            Interactable::Building(b),
        ));
        if b == Building::Kitchen {
            e.insert(KitchenSprite);
            for wx in [-17.5, 16.5] {
                commands.spawn((
                    Transform::from_xyz(foot.x + wx, foot.y + 20.0, 0.0),
                    crate::env::LightSource { radius: 30.0, intensity: 0.8, flicker: true, phase: wx },
                ));
            }
            commands.spawn((
                Chimney,
                Transform::from_xyz(foot.x + 17.0, foot.y + 58.0, 0.0),
                Visibility::Hidden,
            ));
        }
        shadow(&mut commands, &art, foot, s.x as f32 * TILE * 0.9);
    }
    for (o, p) in island.signs.clone() {
        let foot = tile_bottom(p);
        commands.spawn((
            Sprite { anchor: Anchor::BottomCenter, ..Sprite::from_image(art.sign.clone()) },
            Transform::from_xyz(foot.x, foot.y + 1.0, ysort(foot.y)),
            Interactable::Sign(o),
        ));
        if let Some(w) = o.who() {
            let c = w.color();
            let text = if w == Who::You { "Твоя зона".to_string() } else { format!("Зона: {}", w.name()) };
            crate::juice::spawn_label(
                &mut commands,
                &art,
                text,
                Color::srgb(c[0] * 0.4 + 0.6, c[1] * 0.4 + 0.6, c[2] * 0.4 + 0.6),
                14.0,
                foot + Vec2::new(0.0, 24.0),
            );
        }
    }
    for (k, p) in island.nature.clone() {
        let foot = tile_bottom(p);
        let seed = (p.x * 31 + p.y * 17) as f32;
        match k.as_str() {
            "tree0" | "tree1" => {
                let idx = if k == "tree0" { 0 } else { 1 };
                commands.spawn((
                    Sprite {
                        anchor: Anchor::Custom(Vec2::new(0.0, -0.5 + 3.0 / 44.0)),
                        ..Sprite::from_atlas_image(
                            art.trees[0].clone(),
                            TextureAtlas { layout: art.trees_layout.clone(), index: idx },
                        )
                    },
                    Transform::from_xyz(foot.x, foot.y, ysort(foot.y)),
                    SeasonalTree(idx),
                    crate::juice::Sway { phase: seed, amp: 0.012 },
                ));
                shadow(&mut commands, &art, foot + Vec2::new(0.0, 2.0), 22.0);
            }
            "bush" => {
                commands.spawn((
                    Sprite { anchor: Anchor::BottomCenter, ..Sprite::from_image(art.bush[0].clone()) },
                    Transform::from_xyz(foot.x, foot.y + 1.0, ysort(foot.y)),
                    SeasonalBush,
                    crate::juice::Sway { phase: seed, amp: 0.02 },
                ));
                shadow(&mut commands, &art, foot + Vec2::new(0.0, 2.0), 14.0);
            }
            _ => {
                commands.spawn((
                    Sprite { anchor: Anchor::BottomCenter, ..Sprite::from_image(art.rock.clone()) },
                    Transform::from_xyz(foot.x, foot.y + 1.0, ysort(foot.y)),
                ));
            }
        }
    }
}

pub fn shadow(commands: &mut Commands, art: &Art, foot: Vec2, width: f32) {
    commands.spawn((
        Sprite {
            custom_size: Some(Vec2::new(width, width * 0.4)),
            ..Sprite::from_image(art.shadow.clone())
        },
        Transform::from_xyz(foot.x, foot.y + 1.0, -6.0),
    ));
}

fn animate_water(
    time: Res<Time>,
    mut q: Query<&mut Sprite, (With<WaterTile>, Without<FoamSprite>)>,
    mut foam: Query<&mut Sprite, With<FoamSprite>>,
    art: Res<Art>,
    mut last: Local<(usize, usize)>,
) {
    let t = time.elapsed_secs();
    let wf = (t * 2.5) as usize % 4;
    if wf != last.0 {
        last.0 = wf;
        for mut s in &mut q {
            if let Some(a) = s.texture_atlas.as_mut() {
                a.index = wf;
            }
        }
    }
    let ff = [0usize, 1, 2, 1][(t * 1.6) as usize % 4];
    if ff != last.1 {
        last.1 = ff;
        for mut s in &mut foam {
            s.image = art.foam[ff].clone();
        }
    }
}

fn apply_season_art(
    clock: Res<SeasonClock>,
    art: Res<Art>,
    mut last: Local<Option<Season>>,
    mut terrain: Query<&mut Sprite, (With<TerrainSprite>, Without<SeasonalTree>, Without<SeasonalBush>)>,
    mut trees: Query<&mut Sprite, (With<SeasonalTree>, Without<TerrainSprite>, Without<SeasonalBush>)>,
    mut bushes: Query<&mut Sprite, (With<SeasonalBush>, Without<TerrainSprite>, Without<SeasonalTree>)>,
) {
    let s = clock.season();
    if *last == Some(s) {
        return;
    }
    *last = Some(s);
    for mut t in &mut terrain {
        t.image = art.terrain[s.idx()].clone();
    }
    for mut t in &mut trees {
        t.image = art.trees_for(s);
    }
    for mut b in &mut bushes {
        b.image = art.bush[s.idx()].clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_parses_and_zones_are_farmable() {
        let isl = Island::parse(MAP_TXT);
        assert_eq!(isl.w, 64);
        assert_eq!(isl.h, 48);
        assert_eq!(isl.zones.len(), 5);
        assert!(!isl.dock.is_empty());
        for z in &isl.zones {
            let mut farmable = 0;
            for y in z.min.y..=z.max.y {
                for x in z.min.x..=z.max.x {
                    if isl.farmable(IVec2::new(x, y)) {
                        farmable += 1;
                    }
                }
            }
            assert!(farmable > 40, "zone {:?} has only {} farmable tiles", z.owner, farmable);
        }
    }

    #[test]
    fn everything_reachable_from_dock() {
        let isl = Island::parse(MAP_TXT);
        let start = *isl.dock.last().unwrap();
        for b in [Building::Kitchen, Building::Well, Building::Shop, Building::Chest, Building::Crate, Building::Board] {
            let f = isl.building_front(b);
            assert!(isl.walkable(f), "front of {:?} blocked", b);
            assert!(isl.find_path(start, f, 20000).is_some(), "{:?} unreachable", b);
        }
        for z in &isl.zones {
            let c = (z.min + z.max) / 2;
            assert!(isl.find_path(start, c, 20000).is_some(), "zone {:?} unreachable", z.owner);
        }
    }

    #[test]
    fn ownership() {
        let isl = Island::parse(MAP_TXT);
        let you = isl.zone(Owner::You).clone();
        assert_eq!(isl.owner((you.min + you.max) / 2), Owner::You);
        let s = isl.zone(Owner::Shared).clone();
        assert_eq!(isl.owner((s.min + s.max) / 2), Owner::Shared);
    }
}
