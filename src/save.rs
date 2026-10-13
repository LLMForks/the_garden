//! Saving/loading and offline progress: while you're away the garden keeps
//! growing and your friends keep tending it. Nothing ever withers.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::window::WindowCloseRequested;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::data::*;
use crate::env::SeasonClock;
use crate::farming::{self, DecorMap, Farm};
use crate::state::Game;
use crate::ui::{Menu, UiState};
use crate::world::Island;
use crate::Player;

const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct SaveData {
    pub version: u32,
    pub saved_at: u64,
    pub game: Game,
    pub farm: Farm,
    pub decor: DecorMap,
    pub clock: SeasonClock,
    pub player: (f32, f32),
}

#[derive(Event)]
pub struct SaveNow;

/// Where to save. `None` disables saving (tests).
#[derive(Resource, Clone)]
pub struct SavePath(pub Option<PathBuf>);

#[derive(Resource, Default)]
struct PendingPlayerPos(Option<Vec2>);

pub fn default_save_path() -> PathBuf {
    // Windows: %APPDATA%\the_garden\save.ron
    if cfg!(windows) {
        if let Some(a) = std::env::var_os("APPDATA") {
            return PathBuf::from(a).join("the_garden").join("save.ron");
        }
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("the_garden").join("save.ron")
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<SaveNow>()
            .init_resource::<PendingPlayerPos>()
            .add_systems(PreStartup, load_game)
            .add_systems(PostStartup, place_player)
            .add_systems(Update, (autosave, save_on_request))
            .add_systems(Last, save_on_exit);
    }
}

fn load_game(
    path: Res<SavePath>,
    mut game: ResMut<Game>,
    mut farm: ResMut<Farm>,
    mut decor: ResMut<DecorMap>,
    mut clock: ResMut<SeasonClock>,
    mut ui: ResMut<UiState>,
    mut pending: ResMut<PendingPlayerPos>,
    island: Res<Island>,
) {
    let Some(p) = &path.0 else {
        return;
    };
    let data = match std::fs::read_to_string(p) {
        Ok(s) => match ron::from_str::<SaveData>(&s) {
            Ok(d) if d.version == VERSION => d,
            Ok(_) | Err(_) => {
                warn!("save file {:?} unreadable, starting fresh (old one kept as .bak)", p);
                let _ = std::fs::rename(p, p.with_extension("ron.bak"));
                ui.open(Menu::Intro);
                return;
            }
        },
        Err(_) => {
            ui.open(Menu::Intro);
            return;
        }
    };
    let elapsed = now().saturating_sub(data.saved_at) as f32;
    *game = data.game;
    *farm = data.farm;
    *decor = data.decor;
    *clock = data.clock;
    pending.0 = Some(Vec2::new(data.player.0, data.player.1));
    // drop decor that would now be out of bounds/invalid
    decor.items.retain(|k, _| island.in_bounds(IVec2::new(k.0, k.1)));
    let lines = offline_progress(&mut game, &mut farm, &island, clock.season(), elapsed);
    if !lines.is_empty() {
        ui.open(Menu::Welcome(lines));
    }
}

/// Simulates time away. Returns the "while you were away" summary.
pub fn offline_progress(game: &mut Game, farm: &mut Farm, island: &Island, season: Season, secs: f32) -> Vec<String> {
    if secs < 90.0 {
        return vec![];
    }
    let mut lines = vec![];
    let mins = (secs / 60.0) as u32;
    lines.push(if mins < 120 {
        format!("Тебя не было {} мин.", mins)
    } else if mins < 60 * 48 {
        format!("Тебя не было {} ч.", mins / 60)
    } else {
        format!("Тебя не было {} дн.", mins / 60 / 24)
    });
    // Mila waters your dry crops once
    let mut watered = 0;
    for p in farm.plots.values_mut() {
        if p.crop.is_some() && !p.watered && p.planter == Who::You {
            p.watered = true;
            watered += 1;
        }
    }
    if watered > 0 {
        lines.push(format!("Мила полила твои грядки: {}", watered));
        game.stats.watered += watered;
    }
    farming::simulate_offline(farm, season, game.skill(Skill::Gardener), secs);
    let ready = farm.plots.values().filter(|p| p.ready() && p.planter == Who::You).count();
    if ready > 0 {
        lines.push(format!("Твой урожай готов к сбору: {} грядок", ready));
    }
    // friends harvested their own beds a few times
    let cycles = ((secs / 240.0) as u32).clamp(0, 6);
    let mut rng = rand::thread_rng();
    let mut harvested = 0;
    for (k, p) in farm.plots.iter_mut() {
        let owner = island.owner(IVec2::new(k.0, k.1));
        if owner == Owner::You || owner == Owner::Shared {
            continue;
        }
        if let Some(c) = p.crop {
            let n = if p.ready() { 1 + cycles.saturating_sub(1) } else { cycles / 2 };
            if n > 0 {
                game.add(Item::Crop(c), n);
                *game.stats.harvested.entry(c).or_insert(0) += n;
                game.stats.harvested_total += n;
                harvested += n;
                p.growth = rng.gen_range(0.2..0.8);
            }
        }
    }
    if harvested > 0 {
        lines.push(format!("Друзья принесли на общий склад: {} овощей", harvested));
    }
    // Tyoma cooks from the surplus
    let dish = Dish::ALL
        .iter()
        .copied()
        .filter(|d| game.dish_unlocked(*d))
        .find(|d| d.recipe().iter().all(|(c, n)| game.count(Item::Crop(*c)) >= n + 5));
    if let Some(d) = dish {
        for (c, n) in d.recipe() {
            game.take(Item::Crop(*c), *n);
        }
        game.add(Item::Dish(d), 1);
        *game.stats.cooked.entry(d).or_insert(0) += 1;
        game.stats.cooked_total += 1;
        lines.push(format!("Тёма приготовил «{}» из излишков", d.name()));
    }
    lines.push("Ничего не завяло — как и всегда".into());
    lines
}

fn place_player(mut pending: ResMut<PendingPlayerPos>, mut q: Query<&mut Transform, With<Player>>, island: Res<Island>) {
    if let Some(p) = pending.0.take() {
        if let Ok(mut t) = q.single_mut() {
            if island.walkable(crate::world::world_to_tile(p + Vec2::Y * 3.0)) {
                t.translation.x = p.x;
                t.translation.y = p.y;
            }
        }
    }
}

fn write_save(
    path: &SavePath,
    game: &Game,
    farm: &Farm,
    decor: &DecorMap,
    clock: &SeasonClock,
    player: Vec2,
) {
    let Some(p) = &path.0 else { return };
    let data = SaveData {
        version: VERSION,
        saved_at: now(),
        game: game.clone(),
        farm: farm.clone(),
        decor: decor.clone(),
        clock: clock.clone(),
        player: (player.x, player.y),
    };
    let Ok(text) = ron::ser::to_string_pretty(&data, ron::ser::PrettyConfig::default()) else {
        warn!("could not serialize save");
        return;
    };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = p.with_extension("ron.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, p);
    } else {
        warn!("could not write save to {:?}", p);
    }
}

#[allow(clippy::too_many_arguments)]
fn autosave(
    time: Res<Time>,
    mut acc: Local<f32>,
    path: Res<SavePath>,
    game: Res<Game>,
    farm: Res<Farm>,
    decor: Res<DecorMap>,
    clock: Res<SeasonClock>,
    player: Query<&Transform, With<Player>>,
) {
    *acc += time.delta_secs();
    if *acc < 20.0 {
        return;
    }
    *acc = 0.0;
    let pp = player.single().map(|t| t.translation.truncate()).unwrap_or_default();
    write_save(&path, &game, &farm, &decor, &clock, pp);
}

#[allow(clippy::too_many_arguments)]
fn save_on_request(
    mut ev: EventReader<SaveNow>,
    mut close: EventReader<WindowCloseRequested>,
    path: Res<SavePath>,
    game: Res<Game>,
    farm: Res<Farm>,
    decor: Res<DecorMap>,
    clock: Res<SeasonClock>,
    player: Query<&Transform, With<Player>>,
) {
    let a = ev.read().count() > 0;
    let b = close.read().count() > 0;
    if a || b {
        let pp = player.single().map(|t| t.translation.truncate()).unwrap_or_default();
        write_save(&path, &game, &farm, &decor, &clock, pp);
    }
}

#[allow(clippy::too_many_arguments)]
fn save_on_exit(
    mut ev: EventReader<AppExit>,
    path: Res<SavePath>,
    game: Res<Game>,
    farm: Res<Farm>,
    decor: Res<DecorMap>,
    clock: Res<SeasonClock>,
    player: Query<&Transform, With<Player>>,
) {
    if ev.read().count() > 0 {
        let pp = player.single().map(|t| t.translation.truncate()).unwrap_or_default();
        write_save(&path, &game, &farm, &decor, &clock, pp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::MAP_TXT;

    #[test]
    fn save_roundtrip() {
        let mut game = Game::default();
        game.add(Item::Dish(Dish::Bread), 2);
        game.coins = 123;
        let mut farm = Farm::default();
        farm.plots.insert((10, 30), farming::Plot { crop: Some(Crop::Carrot), growth: 0.5, watered: true, planter: Who::You });
        let mut decor = DecorMap::default();
        decor.items.insert((12, 31), (Decor::Lantern, Who::You));
        let data = SaveData {
            version: VERSION,
            saved_at: 1,
            game,
            farm,
            decor,
            clock: SeasonClock::default(),
            player: (1.0, -2.0),
        };
        let s = ron::ser::to_string_pretty(&data, ron::ser::PrettyConfig::default()).unwrap();
        let back: SaveData = ron::from_str(&s).unwrap();
        assert_eq!(back.game.coins, 123);
        assert_eq!(back.game.count(Item::Dish(Dish::Bread)), 2);
        assert_eq!(back.farm.plots[&(10, 30)].crop, Some(Crop::Carrot));
        assert_eq!(back.decor.items[&(12, 31)].0, Decor::Lantern);
    }

    #[test]
    fn loads_saves_from_the_first_beta() {
        let s = include_str!("testdata_old_save.ron");
        let d: SaveData = ron::from_str(s).expect("old save must keep loading");
        assert_eq!(d.version, VERSION);
        assert_eq!(d.game.level, 1);
    }

    #[test]
    fn offline_progress_grows_and_reports() {
        let isl = Island::parse(MAP_TXT);
        let mut game = Game::default();
        let mut farm = Farm::default();
        farm.plots.insert((10, 33), farming::Plot { crop: Some(Crop::Wheat), growth: 0.1, watered: false, planter: Who::You });
        let lines = offline_progress(&mut game, &mut farm, &isl, Season::Spring, 3.0 * 3600.0);
        assert!(farm.plots[&(10, 33)].ready());
        assert!(lines.iter().any(|l| l.contains("завяло")));
        assert!(offline_progress(&mut game, &mut farm, &isl, Season::Spring, 10.0).is_empty());
    }
}
