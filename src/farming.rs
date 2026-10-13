//! Plots, crops, decor and the rules for who may do what where.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

use crate::assets::Art;
use crate::data::*;
use crate::env::SeasonClock;
use crate::juice::{Punch, Sway};
use crate::state::{Game, Note};
use crate::world::{tile_bottom, tile_center, Island};
use crate::{ysort, GameSpeed};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Plot {
    pub crop: Option<Crop>,
    pub growth: f32,
    pub watered: bool,
    pub planter: Who,
}

impl Plot {
    pub fn ready(&self) -> bool {
        self.crop.is_some() && self.growth >= 1.0
    }
    pub fn stage(&self) -> usize {
        stage_of(self.growth)
    }
}

pub fn stage_of(g: f32) -> usize {
    if g < 0.12 {
        0
    } else if g < 0.45 {
        1
    } else if g < 1.0 {
        2
    } else {
        3
    }
}

#[derive(Resource, Default, Serialize, Deserialize, Clone, Debug)]
pub struct Farm {
    pub plots: BTreeMap<(i32, i32), Plot>,
}

impl Farm {
    pub fn get(&self, p: IVec2) -> Option<&Plot> {
        self.plots.get(&(p.x, p.y))
    }
    pub fn get_mut(&mut self, p: IVec2) -> Option<&mut Plot> {
        self.plots.get_mut(&(p.x, p.y))
    }
}

#[derive(Resource, Default, Serialize, Deserialize, Clone, Debug)]
pub struct DecorMap {
    pub items: BTreeMap<(i32, i32), (Decor, Who)>,
}

impl DecorMap {
    pub fn cozy(&self, decorator_lvl: u32) -> u32 {
        let base: u32 = self.items.values().map(|(d, _)| d.cozy()).sum();
        (base as f32 * (1.0 + 0.1 * (decorator_lvl.saturating_sub(1)) as f32)).round() as u32
    }
}

/// Growth per second for a crop.
pub fn growth_rate(crop: Crop, season: Season, watered: bool, gardener_lvl: u32) -> f32 {
    let water = if watered { 2.0 } else { 1.0 };
    let skill = 1.0 + 0.05 * gardener_lvl.saturating_sub(1) as f32;
    season.growth_bonus(crop) * water * skill / crop.grow_secs()
}

#[derive(Debug, Clone, PartialEq)]
pub enum Deny {
    NotYours(Owner),
    CantTill,
    NoSeeds(Crop),
    Locked(u32),
    NoWater,
    Occupied,
    NotReady,
    Nothing,
    AlreadyWet,
}

impl Deny {
    pub fn text(&self) -> String {
        match self {
            Deny::NotYours(o) => match o.who() {
                Some(w) => format!("Это личная зона: {}. Строить тут может только хозяин", w.name()),
                None => "Здесь нельзя".into(),
            },
            Deny::CantTill => "Тут не вскопать — нужна трава".into(),
            Deny::NoSeeds(c) => format!("Нет семян: {} — загляни в лавку", c.name().to_lowercase()),
            Deny::Locked(l) => format!("Откроется на {} уровне острова", l),
            Deny::NoWater => "Лейка пуста — набери воды у колодца или у берега".into(),
            Deny::Occupied => "Место занято".into(),
            Deny::NotReady => "Ещё растёт…".into(),
            Deny::Nothing => "Тут нечего делать".into(),
            Deny::AlreadyWet => "Уже полито".into(),
        }
    }
}

pub fn till(farm: &mut Farm, isl: &Island, decor: &DecorMap, who: Who, p: IVec2) -> Result<(), Deny> {
    if farm.get(p).is_some() || decor.items.contains_key(&(p.x, p.y)) {
        return Err(Deny::Occupied);
    }
    if !isl.farmable(p) {
        return Err(Deny::CantTill);
    }
    let o = isl.owner(p);
    if !o.can_build(who) {
        return Err(Deny::NotYours(o));
    }
    farm.plots.insert((p.x, p.y), Plot { crop: None, growth: 0.0, watered: false, planter: who });
    Ok(())
}

pub fn plant(
    farm: &mut Farm,
    isl: &Island,
    game: &mut Game,
    who: Who,
    p: IVec2,
    crop: Crop,
) -> Result<(), Deny> {
    let o = isl.owner(p);
    let Some(plot) = farm.get(p) else {
        return Err(Deny::Nothing);
    };
    if plot.crop.is_some() {
        return Err(Deny::Occupied);
    }
    if !o.can_build(who) {
        return Err(Deny::NotYours(o));
    }
    if who == Who::You {
        if !game.crop_unlocked(crop) {
            return Err(Deny::Locked(crop.unlock_level()));
        }
        if !game.take(Item::Seeds(crop), 1) {
            return Err(Deny::NoSeeds(crop));
        }
    }
    let plot = farm.get_mut(p).unwrap();
    plot.crop = Some(crop);
    plot.growth = 0.0;
    plot.planter = who;
    Ok(())
}

/// Anyone may water any plot — even in a friend's personal zone. That's the point.
pub fn water(farm: &mut Farm, p: IVec2) -> Result<Option<Who>, Deny> {
    let Some(plot) = farm.get_mut(p) else {
        return Err(Deny::Nothing);
    };
    if plot.watered {
        return Err(Deny::AlreadyWet);
    }
    plot.watered = true;
    Ok(plot.crop.map(|_| plot.planter))
}

pub struct HarvestResult {
    pub crop: Crop,
    pub amount: u32,
    pub seeds: u32,
}

pub fn harvest(
    farm: &mut Farm,
    isl: &Island,
    game: &mut Game,
    who: Who,
    p: IVec2,
    rng: &mut impl Rng,
) -> Result<HarvestResult, Deny> {
    let o = isl.owner(p);
    let Some(plot) = farm.get(p) else {
        return Err(Deny::Nothing);
    };
    let Some(crop) = plot.crop else {
        return Err(Deny::Nothing);
    };
    if !plot.ready() {
        return Err(Deny::NotReady);
    }
    if !o.can_build(who) {
        return Err(Deny::NotYours(o));
    }
    let lvl = if who == Who::You { game.skill(Skill::Gardener) } else { 2 };
    let double = 0.10 + 0.03 * lvl.saturating_sub(1) as f32;
    let amount = if rng.gen::<f32>() < double { 2 } else { 1 };
    // seeds come back so the garden sustains itself (only for you; friends bring their own)
    let seeds = if who == Who::You { 1 + (rng.gen::<f32>() < 0.4) as u32 } else { 0 };
    game.add(Item::Crop(crop), amount);
    if seeds > 0 {
        game.add(Item::Seeds(crop), seeds);
    }
    *game.stats.harvested.entry(crop).or_insert(0) += amount;
    game.stats.harvested_total += amount;
    let plot = farm.get_mut(p).unwrap();
    plot.crop = None;
    plot.growth = 0.0;
    Ok(HarvestResult { crop, amount, seeds })
}

pub fn place_decor(
    dm: &mut DecorMap,
    farm: &Farm,
    isl: &mut Island,
    game: &mut Game,
    who: Who,
    p: IVec2,
    d: Decor,
) -> Result<(), Deny> {
    if farm.get(p).is_some() || dm.items.contains_key(&(p.x, p.y)) {
        return Err(Deny::Occupied);
    }
    if !isl.decor_ok(p) {
        return Err(Deny::Occupied);
    }
    let o = isl.owner(p);
    if !o.can_build(who) {
        return Err(Deny::NotYours(o));
    }
    if who == Who::You && !game.take(Item::Decor(d), 1) {
        return Err(Deny::Nothing);
    }
    dm.items.insert((p.x, p.y), (d, who));
    let i = isl.idx(p);
    isl.decor_solid[i] = true;
    if who == Who::You {
        game.stats.placed += 1;
    }
    Ok(())
}

pub fn pickup_decor(dm: &mut DecorMap, isl: &mut Island, game: &mut Game, who: Who, p: IVec2) -> Result<Decor, Deny> {
    let Some((d, placer)) = dm.items.get(&(p.x, p.y)).copied() else {
        return Err(Deny::Nothing);
    };
    let o = isl.owner(p);
    if !o.can_build(who) {
        return Err(Deny::NotYours(o));
    }
    if o == Owner::Shared && placer != who && placer != Who::You {
        // a friend's gift in the shared zone stays where they put it
        return Err(Deny::NotYours(placer.zone()));
    }
    dm.items.remove(&(p.x, p.y));
    let i = isl.idx(p);
    isl.decor_solid[i] = false;
    if who == Who::You {
        game.add(Item::Decor(d), 1);
    }
    Ok(d)
}

/// Visual / audio feedback request emitted by any action (player or friends).
#[derive(Event, Clone, Debug)]
pub struct Fx {
    pub kind: FxKind,
    pub tile: IVec2,
    pub who: Who,
}

#[derive(Clone, Debug)]
pub enum FxKind {
    Till,
    Plant(Crop),
    Water,
    Harvest(Crop, u32),
    Place(Decor),
    Pickup,
    Deny(String),
    Refill,
}

#[derive(Component)]
pub struct SoilVis(pub IVec2);
#[derive(Component)]
pub struct CropVis {
    pub tile: IVec2,
    pub stage: usize,
    pub crop: Option<Crop>,
}
#[derive(Component)]
pub struct DecorVis(pub IVec2);
#[derive(Component)]
pub struct GlowLight {
    pub base: f32,
    pub phase: f32,
}

#[derive(Resource, Default)]
pub struct PlotEntities {
    pub map: HashMap<IVec2, (Entity, Entity)>,
    pub decor: HashMap<IVec2, (Entity, Option<Entity>)>,
}

pub struct FarmingPlugin;

impl Plugin for FarmingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Farm>()
            .init_resource::<DecorMap>()
            .init_resource::<PlotEntities>()
            .add_event::<Fx>()
            .add_systems(Update, (grow, sync_plots, sync_decor, update_cozy, punch_on_fx).chain());
    }
}

fn grow(time: Res<Time>, speed: Res<GameSpeed>, clock: Res<SeasonClock>, game: Res<Game>, mut farm: ResMut<Farm>) {
    let dt = time.delta_secs() * speed.0;
    let season = clock.season();
    let lvl = game.skill(Skill::Gardener);
    for plot in farm.plots.values_mut() {
        if let Some(c) = plot.crop {
            if plot.growth < 1.0 {
                let l = if plot.planter == Who::You { lvl } else { 2 };
                plot.growth = (plot.growth + dt * growth_rate(c, season, plot.watered, l)).min(1.0);
            }
        }
    }
}

fn sync_plots(
    mut commands: Commands,
    art: Res<Art>,
    farm: Res<Farm>,
    mut ents: ResMut<PlotEntities>,
    mut soils: Query<&mut Sprite, (With<SoilVis>, Without<CropVis>)>,
    mut crops: Query<(Entity, &mut Sprite, &mut CropVis, &mut Visibility), Without<SoilVis>>,
) {
    // remove visuals of plots that no longer exist
    let gone: Vec<IVec2> = ents.map.keys().filter(|p| farm.get(**p).is_none()).copied().collect();
    for p in gone {
        let (a, b) = ents.map.remove(&p).unwrap();
        commands.entity(a).despawn();
        commands.entity(b).despawn();
    }
    for (&(x, y), plot) in &farm.plots {
        let p = IVec2::new(x, y);
        let Some(&(soil, crop)) = ents.map.get(&p) else {
            let c = tile_center(p);
            let soil = commands
                .spawn((
                    Sprite::from_atlas_image(
                        art.soil.clone(),
                        TextureAtlas { layout: art.soil_layout.clone(), index: plot.watered as usize },
                    ),
                    Transform::from_xyz(c.x, c.y, -5.0),
                    SoilVis(p),
                    Punch::new(0.35),
                ))
                .id();
            let b = tile_bottom(p);
            let crop = commands
                .spawn((
                    Sprite {
                        anchor: Anchor::BottomCenter,
                        ..Sprite::from_atlas_image(
                            art.crops.clone(),
                            TextureAtlas { layout: art.crops_layout.clone(), index: 0 },
                        )
                    },
                    Transform::from_xyz(b.x, b.y + 1.0, ysort(b.y)),
                    CropVis { tile: p, stage: 99, crop: None },
                    Visibility::Hidden,
                    Sway { phase: (x * 13 + y * 7) as f32 * 0.37, amp: 0.05 },
                ))
                .id();
            ents.map.insert(p, (soil, crop));
            continue;
        };
        if let Ok(mut s) = soils.get_mut(soil) {
            if let Some(a) = s.texture_atlas.as_mut() {
                let want = plot.watered as usize;
                if a.index != want {
                    a.index = want;
                }
            }
        }
        if let Ok((e, mut s, mut vis, mut v)) = crops.get_mut(crop) {
            let stage = plot.stage();
            if vis.crop != plot.crop || vis.stage != stage {
                let first = vis.stage == 99;
                vis.crop = plot.crop;
                vis.stage = stage;
                match plot.crop {
                    Some(c) => {
                        *v = Visibility::Inherited;
                        if let Some(a) = s.texture_atlas.as_mut() {
                            a.index = c.sprite_row() * 4 + stage;
                        }
                        if !first {
                            commands.entity(e).insert(Punch::new(if stage == 3 { 0.5 } else { 0.3 }));
                        }
                    }
                    None => *v = Visibility::Hidden,
                }
            }
        }
    }
}

fn sync_decor(
    mut commands: Commands,
    art: Res<Art>,
    dm: Res<DecorMap>,
    mut ents: ResMut<PlotEntities>,
    mut island: ResMut<Island>,
) {
    if !dm.is_changed() {
        return;
    }
    let gone: Vec<IVec2> = ents.decor.keys().filter(|p| !dm.items.contains_key(&(p.x, p.y))).copied().collect();
    for p in gone {
        let (a, l) = ents.decor.remove(&p).unwrap();
        commands.entity(a).despawn();
        if let Some(l) = l {
            commands.entity(l).despawn();
        }
        let i = island.idx(p);
        island.decor_solid[i] = false;
    }
    for (&(x, y), (d, _)) in &dm.items {
        let p = IVec2::new(x, y);
        if ents.decor.contains_key(&p) {
            continue;
        }
        let i = island.idx(p);
        island.decor_solid[i] = true;
        let b = tile_bottom(p);
        let e = commands
            .spawn((
                Sprite { anchor: Anchor::BottomCenter, ..Sprite::from_image(art.decor(*d)) },
                Transform::from_xyz(b.x, b.y + 1.0, ysort(b.y)),
                DecorVis(p),
                Punch::new(0.6),
            ))
            .id();
        let light = if d.glows() {
            let h = if *d == Decor::Lantern { 16.0 } else { 6.0 };
            Some(
                commands
                    .spawn((
                        Sprite {
                            color: Color::srgba(1.0, 0.8, 0.45, 0.0),
                            custom_size: Some(Vec2::splat(if *d == Decor::Lantern { 72.0 } else { 48.0 })),
                            ..Sprite::from_image(art.light.clone())
                        },
                        Transform::from_xyz(b.x, b.y + h, 61.0),
                        GlowLight { base: 0.55, phase: (x + y) as f32 },
                    ))
                    .id(),
            )
        } else {
            None
        };
        ents.decor.insert(p, (e, light));
    }
}

fn update_cozy(dm: Res<DecorMap>, mut game: ResMut<Game>) {
    let c = dm.cozy(game.skill(Skill::Decorator));
    if game.cozy != c {
        game.cozy = c;
    }
}

fn punch_on_fx(mut commands: Commands, mut fx: EventReader<Fx>, ents: Res<PlotEntities>) {
    for f in fx.read() {
        if let Some((soil, crop)) = ents.map.get(&f.tile) {
            match f.kind {
                FxKind::Water | FxKind::Till => {
                    commands.entity(*soil).insert(Punch::new(0.3));
                    commands.entity(*crop).insert(Punch::new(0.25));
                }
                FxKind::Plant(_) => {
                    commands.entity(*soil).insert(Punch::new(0.2));
                }
                _ => {}
            }
        }
    }
}

/// Simulates `secs` of growth (used for offline progress — nothing ever withers).
pub fn simulate_offline(farm: &mut Farm, season: Season, gardener_lvl: u32, secs: f32) {
    for plot in farm.plots.values_mut() {
        if let Some(c) = plot.crop {
            let l = if plot.planter == Who::You { gardener_lvl } else { 2 };
            plot.growth = (plot.growth + secs * growth_rate(c, season, plot.watered, l)).min(1.0);
        }
    }
}

pub fn feed_for_water(who: Who, owner: Option<Who>, crop: Option<Crop>, notes: &mut EventWriter<Note>) {
    if let (Some(o), Some(c)) = (owner, crop) {
        if o != who {
            let text = match (who, o) {
                (Who::You, f) => format!("Ты полил(а) {} у {} — дружба крепнет", c.name_gen(), f.name_gen()),
                (f, Who::You) => format!("{} полил(а) твою {}", f.name(), c.name_gen()),
                (f, g) => format!("{} полил(а) {} у {}", f.name(), c.name_gen(), g.name_gen()),
            };
            notes.write(Note::Feed(text));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::MAP_TXT;
    use rand::SeedableRng;

    fn setup() -> (Farm, Island, Game, DecorMap) {
        (Farm::default(), Island::parse(MAP_TXT), Game::default(), DecorMap::default())
    }

    fn your_tile(isl: &Island) -> IVec2 {
        let z = isl.zone(Owner::You);
        for y in z.min.y..=z.max.y {
            for x in z.min.x..=z.max.x {
                let p = IVec2::new(x, y);
                if isl.farmable(p) {
                    return p;
                }
            }
        }
        panic!("no farmable tile")
    }

    #[test]
    fn full_crop_cycle() {
        let (mut farm, isl, mut game, dm) = setup();
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let p = your_tile(&isl);
        till(&mut farm, &isl, &dm, Who::You, p).unwrap();
        assert_eq!(till(&mut farm, &isl, &dm, Who::You, p), Err(Deny::Occupied));
        let seeds = game.count(Item::Seeds(Crop::Wheat));
        plant(&mut farm, &isl, &mut game, Who::You, p, Crop::Wheat).unwrap();
        assert_eq!(game.count(Item::Seeds(Crop::Wheat)), seeds - 1);
        assert!(matches!(harvest(&mut farm, &isl, &mut game, Who::You, p, &mut rng), Err(Deny::NotReady)));
        water(&mut farm, p).unwrap();
        assert_eq!(water(&mut farm, p), Err(Deny::AlreadyWet));
        // watered wheat grows twice as fast
        let rate = growth_rate(Crop::Wheat, Season::Spring, true, 1);
        assert!((rate * Crop::Wheat.grow_secs() - 2.0).abs() < 1e-4);
        simulate_offline(&mut farm, Season::Spring, 1, 10_000.0);
        assert!(farm.get(p).unwrap().ready());
        let r = harvest(&mut farm, &isl, &mut game, Who::You, p, &mut rng).unwrap();
        assert!(r.amount >= 1 && r.seeds >= 1);
        assert_eq!(game.count(Item::Crop(Crop::Wheat)), r.amount);
        assert!(farm.get(p).unwrap().crop.is_none(), "plot stays tilled for replanting");
    }

    #[test]
    fn crops_never_wither() {
        let (mut farm, isl, mut game, dm) = setup();
        let p = your_tile(&isl);
        till(&mut farm, &isl, &dm, Who::You, p).unwrap();
        plant(&mut farm, &isl, &mut game, Who::You, p, Crop::Carrot).unwrap();
        // a whole year away
        simulate_offline(&mut farm, Season::Summer, 1, 365.0 * 24.0 * 3600.0);
        let plot = farm.get(p).unwrap();
        assert_eq!(plot.crop, Some(Crop::Carrot));
        assert!(plot.ready());
    }

    #[test]
    fn personal_zones_are_protected() {
        let (mut farm, isl, mut game, dm) = setup();
        let p = your_tile(&isl);
        assert_eq!(till(&mut farm, &isl, &dm, Who::Mila, p), Err(Deny::NotYours(Owner::You)));
        till(&mut farm, &isl, &dm, Who::You, p).unwrap();
        plant(&mut farm, &isl, &mut game, Who::You, p, Crop::Wheat).unwrap();
        // friends may water your crops...
        assert_eq!(water(&mut farm, p), Ok(Some(Who::You)));
        simulate_offline(&mut farm, Season::Summer, 1, 10_000.0);
        // ...but never harvest them
        let mut rng = rand::rngs::StdRng::seed_from_u64(2);
        assert!(matches!(
            harvest(&mut farm, &isl, &mut game, Who::Mila, p, &mut rng),
            Err(Deny::NotYours(Owner::You))
        ));
    }

    #[test]
    fn locked_crops_cannot_be_planted() {
        let (mut farm, isl, mut game, dm) = setup();
        let p = your_tile(&isl);
        till(&mut farm, &isl, &dm, Who::You, p).unwrap();
        game.add(Item::Seeds(Crop::Potato), 1);
        assert_eq!(plant(&mut farm, &isl, &mut game, Who::You, p, Crop::Potato), Err(Deny::Locked(3)));
    }

    #[test]
    fn decor_place_and_pickup() {
        let (farm, mut isl, mut game, mut dm) = setup();
        let p = your_tile(&isl);
        let n = game.count(Item::Decor(Decor::Fence));
        place_decor(&mut dm, &farm, &mut isl, &mut game, Who::You, p, Decor::Fence).unwrap();
        assert!(!isl.walkable(p));
        assert_eq!(game.count(Item::Decor(Decor::Fence)), n - 1);
        assert_eq!(dm.cozy(1), 1);
        pickup_decor(&mut dm, &mut isl, &mut game, Who::You, p).unwrap();
        assert!(isl.walkable(p));
        assert_eq!(game.count(Item::Decor(Decor::Fence)), n);
    }
}
