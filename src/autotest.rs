//! `--autotest`: a scripted playtest bot. It drives the game through the real
//! keyboard input path (presses keys in `ButtonInput`), checks that the core
//! loop works end to end and optionally captures screenshots. Exits with
//! code 0 on success, 1 on failure.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::path::PathBuf;

use crate::data::*;
use crate::env::{SeasonClock, Weather};
use crate::farming::{self, DecorMap, Farm};
use crate::npc::Npc;
use crate::player::{CharAnim, Facing};
use crate::state::Game;
use crate::ui::{Act, Menu, UiState};
use crate::world::{tile_center, Island};
use crate::{GameSpeed, Player};

pub struct AutotestPlugin {
    pub shots: Option<PathBuf>,
}

#[derive(Resource)]
pub struct NoMouse;

#[derive(Resource)]
struct Bot {
    step: usize,
    t: f32,
    total: f32,
    shots: Option<PathBuf>,
    start_tile: IVec2,
    log: Vec<String>,
    release: Vec<KeyCode>,
}

impl Plugin for AutotestPlugin {
    fn build(&self, app: &mut App) {
        if let Some(d) = &self.shots {
            let _ = std::fs::create_dir_all(d);
        }
        app.insert_resource(NoMouse)
            .insert_resource(Bot {
                step: 0,
                t: 0.0,
                total: 0.0,
                shots: self.shots.clone(),
                start_tile: IVec2::ZERO,
                log: vec![],
                release: vec![],
            })
            .add_systems(PreUpdate, bot.after(bevy::input::InputSystem));
    }
}

fn shot(commands: &mut Commands, bot: &Bot, name: &str) {
    if let Some(d) = &bot.shots {
        let p = d.join(name);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(p));
    }
}

#[allow(clippy::too_many_arguments)]
fn bot(
    mut commands: Commands,
    time: Res<Time>,
    mut bot: ResMut<Bot>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut game: ResMut<Game>,
    mut farm: ResMut<Farm>,
    mut dm: ResMut<DecorMap>,
    mut island: ResMut<Island>,
    mut clock: ResMut<SeasonClock>,
    mut speed: ResMut<GameSpeed>,
    mut ui: ResMut<UiState>,
    mut player: Query<(&mut Transform, &mut CharAnim), With<Player>>,
    npcs: Query<(&Npc, &Visibility)>,
    mut buttons: Query<(&mut Interaction, &Act)>,
    mut exit: EventWriter<AppExit>,
) {
    let dt = time.delta_secs();
    bot.t += dt;
    bot.total += dt;
    for k in std::mem::take(&mut bot.release) {
        keys.release(k);
    }
    mouse.release(MouseButton::Right);
    let mut tap = |bot: &mut Bot, k: KeyCode| {
        keys.press(k);
        bot.release.push(k);
    };
    let fail = |bot: &Bot, exit: &mut EventWriter<AppExit>, msg: &str| {
        eprintln!("AUTOTEST FAILED at step {}: {}", bot.step, msg);
        for l in &bot.log {
            eprintln!("  ok: {}", l);
        }
        exit.write(AppExit::error());
    };
    if bot.total > 150.0 {
        fail(&bot, &mut exit, "timeout");
        bot.step = 999;
        return;
    }
    let Ok((mut ptr, mut anim)) = player.single_mut() else { return };
    let ptile = crate::player::feet_tile(&ptr);
    let next = |bot: &mut Bot, msg: &str| {
        bot.log.push(msg.to_string());
        println!("autotest: {}", msg);
        bot.step += 1;
        bot.t = 0.0;
    };
    match bot.step {
        0 => {
            if bot.t > 1.5 {
                bot.start_tile = ptile;
                shot(&mut commands, &bot, "01_start.png");
                next(&mut bot, "world loaded");
            }
        }
        1 => {
            if bot.t > 0.6 {
                anim.facing = Facing::Down;
                tap(&mut bot, KeyCode::Digit1);
                next(&mut bot, "selected hoe");
            }
        }
        2 => {
            if bot.t > 0.2 {
                tap(&mut bot, KeyCode::Space);
                next(&mut bot, "pressed space (till)");
            }
        }
        3 => {
            if bot.t > 0.3 {
                let t = bot.start_tile + IVec2::Y;
                if farm.get(t).is_none() {
                    return fail(&bot, &mut exit, &format!("tile {:?} was not tilled", t));
                }
                tap(&mut bot, KeyCode::Digit3);
                next(&mut bot, "tilled the soil");
            }
        }
        4 => {
            if bot.t > 0.25 {
                tap(&mut bot, KeyCode::Space);
                next(&mut bot, "pressed space (plant)");
            }
        }
        5 => {
            if bot.t > 0.3 {
                let t = bot.start_tile + IVec2::Y;
                if farm.get(t).and_then(|p| p.crop) != Some(Crop::Wheat) {
                    return fail(&bot, &mut exit, "wheat was not planted");
                }
                tap(&mut bot, KeyCode::Digit2);
                next(&mut bot, "planted wheat");
            }
        }
        6 => {
            if bot.t > 0.25 {
                tap(&mut bot, KeyCode::Space);
                next(&mut bot, "pressed space (water)");
            }
        }
        7 => {
            if bot.t > 0.3 {
                let t = bot.start_tile + IVec2::Y;
                if !farm.get(t).is_some_and(|p| p.watered) || game.can_water != CAN_CAPACITY - 1 {
                    return fail(&bot, &mut exit, "plot was not watered");
                }
                tap(&mut bot, KeyCode::Digit1);
                next(&mut bot, "watered");
            }
        }
        8 => {
            // paint-till while walking left with space held
            keys.press(KeyCode::Space);
            keys.press(KeyCode::KeyA);
            if bot.t > 1.0 {
                keys.release(KeyCode::Space);
                keys.release(KeyCode::KeyA);
                if farm.plots.len() < 3 {
                    return fail(&bot, &mut exit, &format!("hold-to-till made only {} plots", farm.plots.len()));
                }
                if ptile == bot.start_tile {
                    return fail(&bot, &mut exit, "player did not move");
                }
                next(&mut bot, "walking + hold-to-till works");
            }
        }
        9 => {
            // fast-forward growth
            speed.0 = 40.0;
            let t = bot.start_tile + IVec2::Y;
            if farm.get(t).is_some_and(|p| p.ready()) {
                speed.0 = 1.0;
                let p = tile_center(bot.start_tile) - Vec2::Y * 4.0;
                ptr.translation.x = p.x;
                ptr.translation.y = p.y;
                anim.facing = Facing::Down;
                next(&mut bot, "wheat grew");
            } else if bot.t > 20.0 {
                return fail(&bot, &mut exit, "wheat never grew");
            }
        }
        10 => {
            if bot.t > 0.4 {
                anim.facing = Facing::Down;
                tap(&mut bot, KeyCode::Space);
                next(&mut bot, "pressed space (harvest)");
            }
        }
        11 => {
            if bot.t > 0.25 {
                if game.stats.harvested_total == 0 || game.count(Item::Crop(Crop::Wheat)) == 0 {
                    return fail(&bot, &mut exit, "harvest did not reach the shared storage");
                }
                shot(&mut commands, &bot, "02_harvest.png");
                next(&mut bot, "harvested into shared storage");
            }
        }
        12 => {
            if bot.t > 0.6 && bot.t < 10.0 {
                // right click on an empty plot returns the grass
                let t = bot.start_tile + IVec2::NEG_X;
                if farm.get(t).is_none_or(|p| p.crop.is_some()) {
                    farm.plots.insert((t.x, t.y), farming::Plot { crop: None, growth: 0.0, watered: false, planter: Who::You });
                }
                anim.facing = Facing::Left;
                mouse.press(MouseButton::Right);
                bot.t = 10.0;
            }
            if bot.t > 10.3 {
                let t = bot.start_tile + IVec2::NEG_X;
                if farm.get(t).is_some() {
                    return fail(&bot, &mut exit, "right click did not return the grass");
                }
                anim.facing = Facing::Down;
                bot.log.push("right click returns grass".into());
                game.add(Item::Crop(Crop::Wheat), 3);
                ui.open(Menu::Kitchen);
                next(&mut bot, "opened kitchen");
            }
        }
        13 => {
            if bot.t > 0.6 {
                shot(&mut commands, &bot, "03_kitchen.png");
                let before = game.count(Item::Dish(Dish::Bread));
                let mut pressed = false;
                for (mut i, a) in &mut buttons {
                    if matches!(a, Act::Cook(Dish::Bread)) {
                        *i = Interaction::Pressed;
                        pressed = true;
                    }
                }
                if !pressed {
                    return fail(&bot, &mut exit, "no bread button in the kitchen");
                }
                bot.log.push(format!("bread before: {}", before));
                next(&mut bot, "clicked cook bread");
            }
        }
        14 => {
            if bot.t > 0.3 {
                if game.stats.cooked.get(&Dish::Bread).copied().unwrap_or(0) == 0 {
                    return fail(&bot, &mut exit, "bread was not cooked");
                }
                ui.menu = None;
                ui.dirty = true;
                // decorate around the player and make it cozy
                for d in [Decor::Lantern, Decor::Flowerbed, Decor::Lantern, Decor::Bench, Decor::Flowerbed, Decor::Mushroom] {
                    game.add(Item::Decor(d), 1);
                }
                let base = bot.start_tile;
                let spots = [IVec2::new(-3, 2), IVec2::new(-2, 3), IVec2::new(0, 3), IVec2::new(-4, 3), IVec2::new(1, 2), IVec2::new(-5, 2)];
                let ds = [Decor::Lantern, Decor::Flowerbed, Decor::Lantern, Decor::Bench, Decor::Flowerbed, Decor::Mushroom];
                let mut placed = 0;
                for (o, d) in spots.iter().zip(ds) {
                    if farming::place_decor(&mut dm, &farm, &mut island, &mut game, Who::You, base + *o, d).is_ok() {
                        placed += 1;
                    }
                }
                if placed < 3 {
                    return fail(&bot, &mut exit, &format!("only {} decor placed", placed));
                }
                // a 3x3 fence ring
                let ring = base + IVec2::new(-5, 5);
                let mut fences = 0;
                for y in 0..3 {
                    for x in 0..3 {
                        if x == 1 && y == 1 {
                            continue;
                        }
                        game.add(Item::Decor(Decor::Fence), 1);
                        let p = ring + IVec2::new(x, y);
                        if farming::place_decor(&mut dm, &farm, &mut island, &mut game, Who::You, p, Decor::Fence).is_ok() {
                            fences += 1;
                        }
                    }
                }
                if fences < 8 || farming::fence_mask(&dm, ring) != 2 | 4 {
                    return fail(&bot, &mut exit, &format!("fence ring broken: {} placed, corner mask {}", fences, farming::fence_mask(&dm, ring)));
                }
                next(&mut bot, "cooked bread, placed decor");
            }
        }
        15 => {
            if bot.t > 1.0 {
                if game.cozy < 6 {
                    return fail(&bot, &mut exit, &format!("cozy too low: {}", game.cozy));
                }
                if game.pets.is_empty() {
                    return fail(&bot, &mut exit, "no pet came despite cozy garden");
                }
                clock.t = DAY_SECS * 0.77;
                next(&mut bot, "a pet moved in");
            }
        }
        16 => {
            if bot.t > 1.5 {
                shot(&mut commands, &bot, "04_evening.png");
                next(&mut bot, "evening");
            }
        }
        17 => {
            if bot.t > 1.0 {
                clock.t = DAY_SECS * 0.9;
                next(&mut bot, "night falls");
            }
        }
        18 => {
            if bot.t > 1.5 {
                shot(&mut commands, &bot, "05_night.png");
                next(&mut bot, "night");
            }
        }
        19 => {
            // fast-forward so friends show up and work their gardens
            if bot.t > 1.0 {
                speed.0 = 12.0;
                clock.t = DAY_SECS * 0.3;
                next(&mut bot, "fast forward");
            }
        }
        20 => {
            let present = npcs.iter().filter(|(_, v)| **v != Visibility::Hidden).count();
            let friend_plots = farm.plots.values().filter(|p| p.planter != Who::You).count();
            if present >= 2 && friend_plots >= 3 {
                speed.0 = 1.0;
                let s = island.zone(Owner::Shared).clone();
                let c = tile_center(IVec2::new((s.min.x + s.max.x) / 2, s.min.y + 6));
                ptr.translation.x = c.x;
                ptr.translation.y = c.y;
                next(&mut bot, &format!("{} friends on the island, {} friend plots", present, friend_plots));
            } else if bot.t > 40.0 {
                return fail(&bot, &mut exit, &format!("friends inactive: present={} plots={}", present, friend_plots));
            }
        }
        21 => {
            if bot.t > 2.0 {
                shot(&mut commands, &bot, "06_shared_zone.png");
                next(&mut bot, "visited shared zone");
            }
        }
        22 => {
            if bot.t > 1.0 {
                game.level = 4;
                ui.open(Menu::Shop);
                next(&mut bot, "open shop");
            }
        }
        23 => {
            if bot.t > 0.8 && bot.t < 5.0 {
                tap(&mut bot, KeyCode::PageDown);
                bot.t = 5.0;
            }
            if bot.t > 5.6 {
                if ui.scroll <= 0.0 {
                    return fail(&bot, &mut exit, "shop menu did not scroll");
                }
                shot(&mut commands, &bot, "07_shop_scrolled.png");
                next(&mut bot, "shop scrolls");
            }
        }
        24 => {
            if bot.t > 1.0 {
                ui.open(Menu::Board);
                next(&mut bot, "open board");
            }
        }
        25 => {
            if bot.t > 0.8 {
                shot(&mut commands, &bot, "08_board.png");
                next(&mut bot, "board");
            }
        }
        26 => {
            if bot.t > 1.0 {
                ui.menu = None;
                ui.dirty = true;
                clock.day = DAYS_PER_SEASON * 3;
                clock.t = DAY_SECS * 0.4;
                clock.weather = Weather::Snow;
                let p = tile_center(bot.start_tile + IVec2::new(-2, 4)) - Vec2::Y * 4.0;
                ptr.translation.x = p.x;
                ptr.translation.y = p.y;
                next(&mut bot, "go to winter");
            }
        }
        27 => {
            if bot.t > 3.0 {
                shot(&mut commands, &bot, "09_winter.png");
                next(&mut bot, "winter");
            }
        }
        28 => {
            if bot.t > 1.0 {
                clock.day = DAYS_PER_SEASON * 2;
                clock.weather = Weather::Rain;
                next(&mut bot, "go to rainy autumn");
            }
        }
        29 => {
            if bot.t > 2.5 {
                shot(&mut commands, &bot, "10_autumn_rain.png");
                next(&mut bot, "autumn");
            }
        }
        30 => {
            if bot.t > 1.5 {
                println!("AUTOTEST PASSED ({} checks)", bot.log.len());
                exit.write(AppExit::Success);
                bot.step = 999;
            }
        }
        _ => {}
    }
}
