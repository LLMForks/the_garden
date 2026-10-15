//! The Garden — cozy co-op farming on one shared island (offline beta).

// Some data fields/helpers are kept for upcoming features (multiplayer, art swaps).
#![allow(dead_code)]
// no console window next to the game on Windows (except in debug builds)
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod assets;
mod autotest;
mod data;
mod env;
mod farming;
mod juice;
mod npc;
mod pets;
mod player;
mod save;
mod state;
mod ui;
mod world;

use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy::window::{PresentMode, WindowResolution};

pub const TILE: f32 = 16.0;

/// Depth for y-sorted world objects: lower on screen = in front.
pub fn ysort(y: f32) -> f32 {
    5.0 + (-y) * 0.0005
}

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct MainCamera;

/// Simulation speed multiplier (1.0 in normal play; tests run faster).
#[derive(Resource)]
pub struct GameSpeed(pub f32);

#[derive(Resource, Default)]
pub struct Paused(pub bool);

#[derive(Default, Clone)]
pub struct Args {
    pub fresh: bool,
    pub save: Option<std::path::PathBuf>,
    pub autotest: bool,
    pub shots: Option<std::path::PathBuf>,
    pub speed: f32,
    pub size: (f32, f32),
}

fn parse_args() -> Args {
    let mut a = Args { speed: 1.0, size: (1280.0, 720.0), ..default() };
    #[allow(unused_mut)]
    let mut raw: Vec<String> = std::env::args().skip(1).collect();
    // in the browser options come from the URL: ?autotest&fresh
    #[cfg(target_arch = "wasm32")]
    if let Some(q) = web_sys::window().and_then(|w| w.location().search().ok()) {
        for part in q.trim_start_matches('?').split('&').filter(|p| !p.is_empty()) {
            raw.push(format!("--{}", part.split('=').next().unwrap_or("")));
        }
    }
    let mut it = raw.into_iter();
    while let Some(s) = it.next() {
        match s.as_str() {
            "--fresh" => a.fresh = true,
            "--save" => a.save = it.next().map(Into::into),
            "--autotest" => a.autotest = true,
            "--screenshots" => a.shots = it.next().map(Into::into),
            "--speed" => a.speed = it.next().and_then(|v| v.parse().ok()).unwrap_or(1.0),
            "--size" => {
                if let Some(v) = it.next() {
                    let p: Vec<f32> = v.split('x').filter_map(|x| x.parse().ok()).collect();
                    if p.len() == 2 {
                        a.size = (p[0], p[1]);
                    }
                }
            }
            "--help" | "-h" => {
                println!(
                    "The Garden (beta)\n\n  --fresh            start a new island (ignores the save)\n  --save PATH        use a custom save file\n  --size WxH         window size (default 1280x720)\n  --autotest         run the automated playtest bot and exit\n  --screenshots DIR  (with --autotest) save screenshots\n"
                );
                std::process::exit(0);
            }
            _ => {}
        }
    }
    a
}

fn main() {
    let args = parse_args();
    let save_path = if args.autotest {
        None
    } else if args.fresh {
        let p = args.save.clone().unwrap_or_else(save::default_save_path);
        save::io::remove(&p);
        Some(p)
    } else {
        Some(args.save.clone().unwrap_or_else(save::default_save_path))
    };

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "The Garden — бета".into(),
                    name: Some("the_garden".into()),
                    resolution: WindowResolution::new(args.size.0, args.size.1),
                    present_mode: PresentMode::AutoVsync,
                    resizable: true,
                    // web: draw into <canvas id="bevy">, follow the page size, keep keys inside the game
                    canvas: Some("#bevy".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: true,
                    ..default()
                }),
                ..default()
            }),
    )
    .insert_resource(ClearColor(Color::srgb(0.18, 0.45, 0.65)))
    .insert_resource(GameSpeed(args.speed))
    .insert_resource(save::SavePath(save_path))
    .init_resource::<Paused>()
    .add_plugins((
        assets::AssetsPlugin,
        state::StatePlugin,
        world::WorldPlugin,
        env::EnvPlugin,
        farming::FarmingPlugin,
        juice::JuicePlugin,
        player::PlayerPlugin,
        npc::NpcPlugin,
        pets::PetsPlugin,
        ui::UiPlugin,
        save::SavePlugin,
    ))
    .add_systems(Startup, spawn_camera);
    if args.autotest {
        app.add_plugins(autotest::AutotestPlugin { shots: args.shots.clone() });
    }
    app.run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        MainCamera,
        Msaa::Off,
        Projection::from(OrthographicProjection {
            scale: 1.0 / 3.0,
            scaling_mode: ScalingMode::WindowSize,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(300.0, -500.0, 100.0),
    ));
}
