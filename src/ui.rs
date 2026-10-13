//! HUD, hotbar, toasts, activity feed, tutorial and the modal menus
//! (kitchen, shop, selling crate, storage chest, goals board, pause).

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use rand::Rng;

use crate::assets::Art;
use crate::data::*;
use crate::env::{SeasonClock, Weather};
use crate::farming::Farm;
use crate::juice::{spawn_particle, AudioSettings, Play, S};
use crate::player::{Hotbar, Interact, Planned, Slot, Target, SLOTS};
use crate::state::{Game, Note};
use crate::world::{Building, Chimney, Island};
use crate::{Paused, Player};

#[derive(Clone, Debug, PartialEq)]
pub enum Menu {
    Kitchen,
    Shop,
    Sell,
    Chest,
    Board,
    Pause,
    Help,
    Intro,
    Welcome(Vec<String>),
}

#[derive(Resource, Default)]
pub struct UiState {
    pub menu: Option<Menu>,
    pub dirty: bool,
}

impl UiState {
    pub fn blocking(&self) -> bool {
        self.menu.is_some()
    }
    pub fn open(&mut self, m: Menu) {
        self.menu = Some(m);
        self.dirty = true;
    }
}

#[derive(Resource, Default)]
pub struct PointerOverUi(pub bool);

#[derive(Component)]
pub struct ChimneySmoke(pub f32);

#[derive(Component, Clone, Debug)]
pub enum Act {
    Close,
    Cook(Dish),
    BuySeeds(Crop, u32),
    BuyDecor(Decor),
    Sell(Item, bool),
    SelectDecor(Decor),
    ToggleSfx,
    ToggleMusic,
    Help,
    SaveQuit,
}

#[derive(Component)]
struct MenuRoot;
#[derive(Component)]
struct HudLevel;
#[derive(Component)]
struct HudXpFill;
#[derive(Component)]
struct HudCoins;
#[derive(Component)]
struct HudCozy;
#[derive(Component)]
struct HudSeason;
#[derive(Component)]
struct HudGoal;
#[derive(Component)]
struct HudGoalFill;
#[derive(Component)]
struct HudTutorial;
#[derive(Component)]
struct HudTutorialText;
#[derive(Component)]
struct HudHint;
#[derive(Component)]
struct HudSlotName;
#[derive(Component)]
struct HotSlot(usize);
#[derive(Component)]
struct HotIcon(usize);
#[derive(Component)]
struct HotCount(usize);
#[derive(Component)]
struct ToastBox;
#[derive(Component)]
struct Toast {
    t: f32,
}
#[derive(Component)]
struct FeedBox;
#[derive(Component)]
struct FeedLine {
    t: f32,
}
#[derive(Component)]
struct HoverInfo;
#[derive(Component)]
struct Banner {
    t: f32,
}
#[derive(Component)]
struct Bump {
    t: f32,
}

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiState>()
            .init_resource::<PointerOverUi>()
            .add_systems(Startup, spawn_hud)
            .add_systems(
                Update,
                (
                    ui_scale,
                    hud_update,
                    hotbar_update,
                    hover_info,
                    interact_hint,
                    notes_to_ui,
                    toasts_anim,
                    open_menus,
                    menu_keys,
                    build_menu,
                    menu_buttons,
                    button_hover,
                    smoke,
                    pointer_over,
                ),
            );
    }
}

// ------------------------------------------------------------------ helpers

fn font(art: &Art, size: f32) -> TextFont {
    TextFont { font: art.font.clone(), font_size: size, ..default() }
}

const INK: Color = Color::srgb(0.22, 0.15, 0.12);
const INK_SOFT: Color = Color::srgb(0.45, 0.36, 0.30);
const GOOD: Color = Color::srgb(0.25, 0.55, 0.22);
const BAD: Color = Color::srgb(0.75, 0.25, 0.2);
const LIGHT: Color = Color::srgb(1.0, 0.98, 0.92);

fn panel_img(art: &Art) -> ImageNode {
    ImageNode {
        image: art.ui_panel.clone(),
        image_mode: NodeImageMode::Sliced(TextureSlicer { border: BorderRect::all(6.0), max_corner_scale: 2.0, ..default() }),
        ..default()
    }
}

fn dark_img(art: &Art) -> ImageNode {
    ImageNode {
        image: art.ui_dark.clone(),
        image_mode: NodeImageMode::Sliced(TextureSlicer { border: BorderRect::all(6.0), max_corner_scale: 2.0, ..default() }),
        ..default()
    }
}

fn button_img(art: &Art, on: bool) -> ImageNode {
    ImageNode {
        image: if on { art.ui_button.clone() } else { art.ui_button_off.clone() },
        image_mode: NodeImageMode::Sliced(TextureSlicer { border: BorderRect::all(4.0), max_corner_scale: 2.0, ..default() }),
        ..default()
    }
}

fn icon_node(art: &Art, idx: usize, size: f32) -> impl Bundle {
    (Node { width: Val::Px(size), height: Val::Px(size), ..default() }, art.icon_node(idx))
}

fn button(p: &mut ChildSpawnerCommands, art: &Art, label: &str, act: Act, on: bool) {
    let mut e = p.spawn((
        Node {
            padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
            margin: UiRect::left(Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            min_width: Val::Px(64.0),
            ..default()
        },
        button_img(art, on),
    ));
    e.with_child((Text::new(label), font(art, 14.0), TextColor(if on { LIGHT } else { Color::srgb(0.95, 0.92, 0.88) }), TextShadow { offset: Vec2::new(1.0, 1.0), color: Color::srgba(0.1, 0.2, 0.05, 0.6) }));
    if on {
        e.insert((Button, act));
    }
}

// ------------------------------------------------------------------ HUD

fn ui_scale(windows: Query<&Window, With<PrimaryWindow>>, mut s: ResMut<UiScale>) {
    if let Ok(w) = windows.single() {
        let want = (w.height() / 720.0).clamp(0.8, 3.0);
        if (s.0 - want).abs() > 0.01 {
            s.0 = want;
        }
    }
}

fn spawn_hud(mut commands: Commands, art: Res<Art>) {
    let art = &*art;
    // top-left: island status
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.0),
                top: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(4.0),
                width: Val::Px(250.0),
                ..default()
            },
            panel_img(art),
        ))
        .with_children(|p| {
            p.spawn((Text::new("Остров · ур. 1"), font(art, 18.0), TextColor(INK), HudLevel));
            p.spawn((
                Node { width: Val::Percent(100.0), height: Val::Px(8.0), ..default() },
                BackgroundColor(Color::srgb(0.85, 0.78, 0.64)),
                BorderRadius::all(Val::Px(3.0)),
            ))
            .with_child((
                Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
                BackgroundColor(Color::srgb(0.45, 0.75, 0.35)),
                BorderRadius::all(Val::Px(3.0)),
                HudXpFill,
            ));
            p.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                r.spawn(icon_node(art, icon::COIN, 24.0));
                r.spawn((Text::new("0"), font(art, 16.0), TextColor(INK), HudCoins));
                r.spawn(Node { width: Val::Px(14.0), ..default() });
                r.spawn(icon_node(art, icon::COZY, 24.0));
                r.spawn((Text::new("Уют 0"), font(art, 16.0), TextColor(INK), HudCozy));
            });
            p.spawn((Text::new(""), font(art, 14.0), TextColor(INK_SOFT), HudSeason));
        });
    // top-right: shared goal
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                top: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(4.0),
                width: Val::Px(270.0),
                ..default()
            },
            panel_img(art),
        ))
        .with_children(|p| {
            p.spawn(Node { column_gap: Val::Px(6.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
                r.spawn(icon_node(art, icon::STAR, 20.0));
                r.spawn((Text::new("Общая цель острова"), font(art, 14.0), TextColor(INK_SOFT)));
            });
            p.spawn((Text::new(""), font(art, 15.0), TextColor(INK), HudGoal));
            p.spawn((
                Node { width: Val::Percent(100.0), height: Val::Px(8.0), ..default() },
                BackgroundColor(Color::srgb(0.85, 0.78, 0.64)),
                BorderRadius::all(Val::Px(3.0)),
            ))
            .with_child((
                Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
                BackgroundColor(Color::srgb(0.95, 0.72, 0.25)),
                BorderRadius::all(Val::Px(3.0)),
                HudGoalFill,
            ));
        });
    // top-center: tutorial + toasts
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Percent(50.0),
                width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)),
                    min_width: Val::Px(340.0),
                    max_width: Val::Px(420.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                dark_img(art),
                HudTutorial,
            ))
            .with_child((Text::new(""), font(art, 15.0), TextColor(LIGHT), TextLayout::new_with_justify(JustifyText::Center), HudTutorialText));
            p.spawn((
                Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(5.0), ..default() },
                ToastBox,
            ));
        });
    // bottom-center: hint + hotbar
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(10.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((
                Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)), ..default() },
                dark_img(art),
                Visibility::Hidden,
                HudHint,
            ))
            .with_child((Text::new(""), font(art, 15.0), TextColor(LIGHT)));
            p.spawn((Text::new(""), font(art, 15.0), TextColor(LIGHT), TextShadow { offset: Vec2::new(1.0, 1.0), color: Color::srgba(0.1, 0.06, 0.14, 0.9) }, HudSlotName));
            p.spawn((Node { column_gap: Val::Px(4.0), padding: UiRect::all(Val::Px(6.0)), ..default() }, panel_img(art)))
                .with_children(|r| {
                    for i in 0..SLOTS.len() {
                        r.spawn((
                            Node {
                                width: Val::Px(52.0),
                                height: Val::Px(52.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            ImageNode::new(art.ui_slot.clone()),
                            Button,
                            HotSlot(i),
                        ))
                        .with_children(|s| {
                            s.spawn((icon_node(art, icon::HOE, 32.0), HotIcon(i)));
                            s.spawn((
                                Text::new(format!("{}", i + 1)),
                                font(art, 11.0),
                                TextColor(INK_SOFT),
                                Node { position_type: PositionType::Absolute, left: Val::Px(4.0), top: Val::Px(2.0), ..default() },
                            ));
                            s.spawn((
                                Text::new(""),
                                font(art, 13.0),
                                TextColor(INK),
                                Node { position_type: PositionType::Absolute, right: Val::Px(4.0), bottom: Val::Px(1.0), ..default() },
                                HotCount(i),
                            ));
                        });
                    }
                });
        });
    // bottom-left: activity feed
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(90.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            max_width: Val::Px(520.0),
            ..default()
        },
        FeedBox,
        Pickable::IGNORE,
    ));
    // bottom-right: key help
    commands.spawn((
        Node { position_type: PositionType::Absolute, right: Val::Px(12.0), bottom: Val::Px(12.0), ..default() },
        Text::new("Esc — меню · H — управление · Tab — склад"),
        font(art, 13.0),
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
        TextShadow { offset: Vec2::new(1.0, 1.0), color: Color::srgba(0.0, 0.0, 0.0, 0.7) },
        Pickable::IGNORE,
    ));
    // hover tooltip
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), ..default() },
            dark_img(art),
            Visibility::Hidden,
            HoverInfo,
            GlobalZIndex(10),
            Pickable::IGNORE,
        ))
        .with_child((Text::new(""), font(art, 13.0), TextColor(LIGHT)));
    // level-up banner
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Percent(28.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            Banner { t: 99.0 },
            Pickable::IGNORE,
        ))
        .with_child((
            Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)), ..default() },
            panel_img(art),
            children![
                (Text::new(""), font(art, 28.0), TextColor(INK)),
                (Text::new(""), font(art, 15.0), TextColor(INK_SOFT), TextLayout::new_with_justify(JustifyText::Center)),
            ],
        ));
}

fn hud_update(
    game: Res<Game>,
    clock: Res<SeasonClock>,
    time: Res<Time>,
    mut texts: ParamSet<(
        Query<&mut Text, With<HudLevel>>,
        Query<&mut Text, With<HudCoins>>,
        Query<&mut Text, With<HudCozy>>,
        Query<&mut Text, With<HudSeason>>,
        Query<&mut Text, With<HudGoal>>,
        Query<&mut Text, With<HudTutorialText>>,
    )>,
    mut fills: ParamSet<(Query<&mut Node, With<HudXpFill>>, Query<&mut Node, With<HudGoalFill>>)>,
    mut tut: Query<&mut Visibility, With<HudTutorial>>,
    mut shown_coins: Local<f32>,
) {
    let target = game.coins as f32;
    *shown_coins += (target - *shown_coins) * (time.delta_secs() * 10.0).min(1.0);
    if (target - *shown_coins).abs() < 0.5 {
        *shown_coins = target;
    }
    if let Ok(mut t) = texts.p0().single_mut() {
        t.0 = format!("Остров · уровень {}", game.level);
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        t.0 = format!("{}", shown_coins.round() as u32);
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        t.0 = format!("Уют {}", game.cozy);
    }
    if let Ok(mut t) = texts.p3().single_mut() {
        let w = match clock.weather {
            Weather::Rain => " · дождь",
            Weather::Snow => " · снег",
            Weather::Clear => "",
        };
        t.0 = format!("{} · день {} · {}{}", clock.season().name(), clock.day_in_season(), clock.clock_text(), w);
    }
    let (goal_text, k) = match game.goal_progress() {
        Some((g, have, need)) => (format!("{}\n{} / {}", g.title, have, need), have as f32 / need as f32),
        None => ("Все цели выполнены! Остров процветает".into(), 1.0),
    };
    if let Ok(mut t) = texts.p4().single_mut() {
        t.0 = goal_text;
    }
    let tut_text = match game.tutorial {
        0 => "Возьми мотыгу [1] и вскопай землю в своей зоне — ЛКМ или Пробел",
        1 => "Выбери семена пшеницы [3] и посади их на грядку",
        2 => "Полей грядку лейкой [2] — политое растёт вдвое быстрее",
        3 => "Пока растёт — осмотрись. Когда заблестит — собери урожай!",
        4 => "Отнеси урожай: продай в ящике у лавки или приготовь блюдо на кухне [E]",
        5 => "Купи украшение в лавке и поставь его [7] — уют приманивает питомцев",
        _ => "",
    };
    if let Ok(mut t) = texts.p5().single_mut() {
        t.0 = tut_text.into();
    }
    if let Ok(mut v) = tut.single_mut() {
        *v = if tut_text.is_empty() { Visibility::Hidden } else { Visibility::Inherited };
    }
    if let Ok(mut n) = fills.p0().single_mut() {
        n.width = Val::Percent(100.0 * game.xp as f32 / xp_for_next(game.level) as f32);
    }
    if let Ok(mut n) = fills.p1().single_mut() {
        n.width = Val::Percent(100.0 * k);
    }
}

fn slot_icon(slot: Slot) -> usize {
    match slot {
        Slot::Hoe => icon::HOE,
        Slot::Can => icon::CAN,
        Slot::Seeds(c) => c.seed_icon(),
        Slot::Decor => 0,
    }
}

#[allow(clippy::type_complexity)]
fn hotbar_update(
    mut commands: Commands,
    hb: Res<Hotbar>,
    game: Res<Game>,
    art: Res<Art>,
    time: Res<Time>,
    mut slots: Query<(Entity, &HotSlot, &mut ImageNode, &mut Node, Option<&mut Bump>), (Without<HotIcon>,)>,
    mut icons: Query<(&HotIcon, &mut ImageNode, &mut Node), Without<HotSlot>>,
    mut counts: Query<(&HotCount, &mut Text, &mut TextColor), Without<HudSlotName>>,
    mut name: Query<&mut Text, (With<HudSlotName>, Without<HotCount>)>,
    mut last_sel: Local<Option<usize>>,
) {
    for (e, s, mut img, mut node, bump) in &mut slots {
        let sel = s.0 == hb.sel;
        img.image = if sel { art.ui_slot_sel.clone() } else { art.ui_slot.clone() };
        if sel && *last_sel != Some(hb.sel) {
            commands.entity(e).insert(Bump { t: 0.0 });
        }
        let lift = match bump {
            Some(mut b) => {
                b.t += time.delta_secs();
                let k = (b.t * 20.0).sin() * (-b.t * 6.0).exp() * 6.0;
                if b.t > 0.8 {
                    commands.entity(e).remove::<Bump>();
                }
                k
            }
            None => 0.0,
        };
        node.top = Val::Px(if sel { -6.0 } else { 0.0 } - lift);
    }
    *last_sel = Some(hb.sel);
    for (i, mut img, mut node) in &mut icons {
        let slot = SLOTS[i.0];
        let locked = matches!(slot, Slot::Seeds(c) if !game.crop_unlocked(c));
        match slot {
            Slot::Decor => {
                img.image = art.decor(hb.decor);
                img.texture_atlas = None;
                let tall = matches!(hb.decor, Decor::Lantern | Decor::Scarecrow | Decor::Birdhouse | Decor::Sunflower | Decor::Snowman);
                node.width = Val::Px(if hb.decor == Decor::Bench { 40.0 } else { if tall { 22.0 } else { 32.0 } });
                node.height = Val::Px(if tall { 33.0 } else { if hb.decor == Decor::Bench { 20.0 } else { 32.0 } });
                img.color = if game.count(Item::Decor(hb.decor)) > 0 { Color::WHITE } else { Color::srgba(1.0, 1.0, 1.0, 0.35) };
            }
            _ => {
                img.image = art.icons.clone();
                img.texture_atlas = Some(TextureAtlas { layout: art.icons_layout.clone(), index: slot_icon(slot) });
                img.color = if locked { Color::srgba(0.3, 0.3, 0.3, 0.5) } else { Color::WHITE };
            }
        }
    }
    for (c, mut t, mut col) in &mut counts {
        let slot = SLOTS[c.0];
        let (s, color) = match slot {
            Slot::Can => (format!("{}", game.can_water), if game.can_water == 0 { BAD } else { Color::srgb(0.2, 0.45, 0.75) }),
            Slot::Seeds(cr) => {
                if game.crop_unlocked(cr) {
                    let n = game.count(Item::Seeds(cr));
                    (format!("{}", n), if n == 0 { BAD } else { INK })
                } else {
                    (format!("ур.{}", cr.unlock_level()), INK_SOFT)
                }
            }
            Slot::Decor => {
                let n = game.count(Item::Decor(hb.decor));
                (format!("{}", n), if n == 0 { BAD } else { INK })
            }
            Slot::Hoe => (String::new(), INK),
        };
        t.0 = s;
        col.0 = color;
    }
    if let Ok(mut t) = name.single_mut() {
        let s = match hb.slot() {
            Slot::Hoe => "Мотыга — вскопать землю".to_string(),
            Slot::Can => format!("Лейка — {} / {} (набрать: у воды или колодца)", game.can_water, CAN_CAPACITY),
            Slot::Seeds(c) => {
                if game.crop_unlocked(c) {
                    format!("Семена: {} ({} шт.)", c.name().to_lowercase(), game.count(Item::Seeds(c)))
                } else {
                    format!("{} — откроется на {} уровне острова", c.name(), c.unlock_level())
                }
            }
            Slot::Decor => format!("{} ×{} — [R] сменить, ПКМ — убрать", hb.decor.name(), game.count(Item::Decor(hb.decor))),
        };
        t.0 = if hb.changed_t < 2.5 { s } else { String::new() };
    }
}

fn hover_info(
    windows: Query<&Window, With<PrimaryWindow>>,
    target: Res<Target>,
    farm: Res<Farm>,
    island: Res<Island>,
    ui: Res<UiState>,
    scale: Res<UiScale>,
    clock: Res<SeasonClock>,
    game: Res<Game>,
    mut q: Query<(&mut Node, &mut Visibility, &Children), With<HoverInfo>>,
    mut texts: Query<&mut Text>,
) {
    let Ok((mut node, mut vis, children)) = q.single_mut() else { return };
    let plot = farm.get(target.tile);
    let show = !ui.blocking() && plot.is_some_and(|p| p.crop.is_some());
    if !show {
        *vis = Visibility::Hidden;
        return;
    }
    let p = plot.unwrap();
    let c = p.crop.unwrap();
    let owner = match p.planter {
        Who::You => "твоя грядка".to_string(),
        w => format!("грядка: {}", w.name()),
    };
    let state = if p.ready() {
        "готово — собирай!".to_string()
    } else {
        let lvl = if p.planter == Who::You { game.skill(Skill::Gardener) } else { 2 };
        let rate = crate::farming::growth_rate(c, clock.season(), p.watered, lvl);
        let secs = ((1.0 - p.growth) / rate).ceil() as u32;
        let w = if p.watered { "полито" } else { "сухо" };
        format!("{}% · ~{}с · {}", (p.growth * 100.0) as u32, secs, w)
    };
    let bonus = if clock.season().growth_bonus(c) > 1.0 { " · сезон +" } else { "" };
    if let Some(t) = children.first() {
        if let Ok(mut t) = texts.get_mut(*t) {
            t.0 = format!("{} ({}) · {}{}", c.name(), owner, state, bonus);
        }
    }
    *vis = Visibility::Inherited;
    let _ = island;
    if let Ok(w) = windows.single() {
        if let (Some(cp), true) = (w.cursor_position(), target.mouse_active) {
            node.left = Val::Px(cp.x / scale.0 + 16.0);
            node.top = Val::Px(cp.y / scale.0 + 14.0);
        } else {
            node.left = Val::Px(w.width() / scale.0 / 2.0 + 20.0);
            node.top = Val::Px(w.height() / scale.0 / 2.0 + 30.0);
        }
    }
}

fn interact_hint(
    player: Query<&Transform, With<Player>>,
    island: Res<Island>,
    ui: Res<UiState>,
    npcs: Query<(&Transform, &crate::player::Talkable, &Visibility), Without<Player>>,
    pets: Query<&Transform, (With<crate::pets::Pet>, Without<Player>)>,
    target: Res<Target>,
    mut q: Query<(&mut Visibility, &Children), (With<HudHint>, Without<crate::player::Talkable>)>,
    mut texts: Query<&mut Text>,
) {
    let Ok((mut vis, ch)) = q.single_mut() else { return };
    let Ok(pt) = player.single() else { return };
    let pp = pt.translation.truncate() + Vec2::Y * 4.0;
    let mut hint: Option<String> = None;
    for t in &pets {
        if t.translation.truncate().distance(pp) < 22.0 {
            hint = Some("[E] Погладить".into());
        }
    }
    for (t, tk, v) in &npcs {
        if *v != Visibility::Hidden && (t.translation.truncate() + Vec2::Y * 4.0).distance(pp) < 24.0 {
            hint = Some(format!("[E] Поболтать: {}", tk.0.name()));
        }
    }
    if hint.is_none() {
        for (b, p, s) in &island.buildings {
            let lo = Vec2::new(p.x as f32 * 16.0, -((p.y + s.y) as f32 * 16.0));
            let hi = Vec2::new((p.x + s.x) as f32 * 16.0, -(p.y as f32 * 16.0));
            if pp.clamp(lo, hi).distance(pp) < 14.0 {
                hint = Some(match b {
                    Building::Kitchen => "[E] Общая кухня — готовить блюда".into(),
                    Building::Shop => "[E] Лавка — семена и украшения".into(),
                    Building::Crate => "[E] Ящик продажи".into(),
                    Building::Chest => "[E] Общий склад".into(),
                    Building::Well => "[E] Колодец — набрать воды".into(),
                    Building::Board => "[E] Доска острова — цели и навыки".into(),
                    Building::Tent(w) => {
                        if *w == Who::You {
                            "Твоя палатка".into()
                        } else {
                            format!("Палатка: {}", w.name())
                        }
                    }
                });
            }
        }
    }
    if hint.is_none() && matches!(target.plan, Planned::Refill) {
        hint = Some("[ЛКМ] Набрать воды".into());
    }
    match (hint, ui.blocking()) {
        (Some(h), false) => {
            *vis = Visibility::Inherited;
            if let Some(c) = ch.first() {
                if let Ok(mut t) = texts.get_mut(*c) {
                    t.0 = h;
                }
            }
        }
        _ => *vis = Visibility::Hidden,
    }
}

fn notes_to_ui(
    mut commands: Commands,
    art: Res<Art>,
    mut notes: EventReader<Note>,
    toast_box: Query<Entity, With<ToastBox>>,
    feed_box: Query<(Entity, Option<&Children>), With<FeedBox>>,
    mut banner: Query<(&mut Banner, &mut Visibility, &Children)>,
    children_q: Query<&Children>,
    mut texts: Query<&mut Text>,
    mut play: EventWriter<Play>,
    player: Query<&Transform, With<Player>>,
    game: Res<Game>,
    clock: Res<SeasonClock>,
) {
    let Ok(tb) = toast_box.single() else { return };
    let Ok((fb, fch)) = feed_box.single() else { return };
    let mut feed_count = fch.map(|c| c.len()).unwrap_or(0);
    for n in notes.read() {
        let toast = |commands: &mut Commands, text: String, icon_idx: usize| {
            commands.entity(tb).with_children(|p| {
                p.spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                        column_gap: Val::Px(8.0),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    panel_img(&art),
                    Toast { t: 0.0 },
                ))
                .with_children(|r| {
                    r.spawn(icon_node(&art, icon_idx, 24.0));
                    r.spawn((Text::new(text), font(&art, 15.0), TextColor(INK)));
                });
            });
        };
        match n {
            Note::Feed(s) => {
                commands.entity(fb).with_child((
                    Text::new(s.clone()),
                    font(&art, 13.0),
                    TextColor(LIGHT),
                    TextShadow { offset: Vec2::new(1.0, 1.0), color: Color::srgba(0.05, 0.03, 0.08, 0.85) },
                    FeedLine { t: 0.0 },
                ));
                feed_count += 1;
                if feed_count > 6 {
                    if let Some(c) = fch.and_then(|c| c.first()) {
                        commands.entity(*c).despawn();
                    }
                }
            }
            Note::Toast(s, i) => {
                toast(&mut commands, s.clone(), *i);
                play.write(Play::ui(S::Open).vol(0.25).pitch(1.4));
            }
            Note::GoalDone(title) => {
                toast(&mut commands, format!("Цель выполнена: {}", title), icon::STAR);
                play.write(Play::ui(S::Quest).vol(0.6));
                play.write(Play::ui(S::Coin).vol(0.4));
            }
            Note::SkillUp(s, l) => {
                toast(&mut commands, format!("Навык «{}» — уровень {}! {}", s.name(), l, s.perk(*l)), icon::STAR);
                play.write(Play::ui(S::Quest).vol(0.5));
            }
            Note::LevelUp(l) => {
                if let Ok((mut b, mut v, ch)) = banner.single_mut() {
                    b.t = 0.0;
                    *v = Visibility::Inherited;
                    let mut unlocks: Vec<String> = vec![];
                    for c in Crop::ALL {
                        if c.unlock_level() == *l {
                            unlocks.push(format!("семена: {}", c.name().to_lowercase()));
                        }
                    }
                    for d in Dish::ALL {
                        if d.unlock_level() == *l {
                            unlocks.push(format!("рецепт: {}", d.name().to_lowercase()));
                        }
                    }
                    for d in Decor::ALL {
                        if d.unlock_level() == *l && d.season().is_none_or(|s| s == clock.season()) {
                            unlocks.push(d.name().to_lowercase());
                        }
                    }
                    let sub = if unlocks.is_empty() {
                        "Весь остров растёт вместе с вами".to_string()
                    } else {
                        format!("Открыто: {}", unlocks.join(", "))
                    };
                    if let Some(inner) = ch.first() {
                        if let Ok(ic) = children_q.get(*inner) {
                            if let Ok(mut t) = texts.get_mut(ic[0]) {
                                t.0 = format!("Остров достиг уровня {}!", l);
                            }
                            if let Ok(mut t) = texts.get_mut(ic[1]) {
                                t.0 = sub;
                            }
                        }
                    }
                }
                play.write(Play::ui(S::LevelUp).vol(0.7));
                // confetti around the player
                if let Ok(pt) = player.single() {
                    let mut rng = rand::thread_rng();
                    let cols = [
                        Color::srgb(1.0, 0.85, 0.3),
                        Color::srgb(0.95, 0.45, 0.55),
                        Color::srgb(0.5, 0.8, 1.0),
                        Color::srgb(0.6, 0.9, 0.45),
                        Color::srgb(0.8, 0.6, 1.0),
                    ];
                    for _ in 0..70 {
                        let a = rng.gen_range(0.0..std::f32::consts::TAU);
                        let v = Vec2::new(a.cos(), a.sin().abs() + 0.4) * rng.gen_range(60.0..160.0);
                        let _ = spawn_particle(
                            &mut commands,
                            pt.translation.truncate() + Vec2::Y * 16.0,
                            56.0,
                            v,
                            cols[rng.gen_range(0..cols.len())],
                            rng.gen_range(1.5..3.0),
                            rng.gen_range(1.0..1.8),
                            140.0,
                        );
                    }
                }
            }
        }
    }
    let _ = game;
}

fn toasts_anim(
    mut commands: Commands,
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut Toast, &mut Node, &mut ImageNode), Without<FeedLine>>,
    mut feed: Query<(Entity, &mut FeedLine, &mut TextColor, &mut TextShadow)>,
    mut banner: Query<(&mut Banner, &mut Visibility, &mut Node), Without<Toast>>,
) {
    let dt = time.delta_secs();
    let n = toasts.iter().count();
    for (i, (e, mut t, mut node, mut img)) in toasts.iter_mut().enumerate() {
        t.t += dt * if n > 3 && i == 0 { 3.0 } else { 1.0 };
        let k_in = (t.t / 0.25).min(1.0);
        let off = (1.0 - k_in).powi(2) * -24.0;
        node.margin.top = Val::Px(off);
        let a = if t.t > 4.0 { (1.0 - (t.t - 4.0) / 0.5).max(0.0) } else { k_in };
        img.color.set_alpha(a);
        if t.t > 4.5 {
            commands.entity(e).despawn();
        }
    }
    for (e, mut f, mut c, mut sh) in &mut feed {
        f.t += dt;
        let a = if f.t > 9.0 { (1.0 - (f.t - 9.0) / 1.0).max(0.0) } else { (f.t / 0.2).min(1.0) };
        c.0.set_alpha(a);
        sh.color.set_alpha(a * 0.85);
        if f.t > 10.0 {
            commands.entity(e).despawn();
        }
    }
    if let Ok((mut b, mut v, mut node)) = banner.single_mut() {
        b.t += dt;
        if b.t > 3.6 && *v != Visibility::Hidden {
            *v = Visibility::Hidden;
        }
        let k = (b.t / 0.35).min(1.0);
        node.top = Val::Percent(22.0 + 6.0 * (1.0 - (1.0 - k).powi(3)));
    }
}

// ------------------------------------------------------------------ menus

fn open_menus(
    mut ev: EventReader<Interact>,
    mut ui: ResMut<UiState>,
    mut game: ResMut<Game>,
    mut play: EventWriter<Play>,
    mut fx: EventWriter<crate::farming::Fx>,
    island: Res<Island>,
    mut notes: EventWriter<Note>,
) {
    for i in ev.read() {
        match i {
            Interact::Building(b) => match b {
                Building::Kitchen => ui.open(Menu::Kitchen),
                Building::Shop => ui.open(Menu::Shop),
                Building::Crate => ui.open(Menu::Sell),
                Building::Chest => ui.open(Menu::Chest),
                Building::Board => ui.open(Menu::Board),
                Building::Well => {
                    game.can_water = CAN_CAPACITY;
                    fx.write(crate::farming::Fx {
                        kind: crate::farming::FxKind::Refill,
                        tile: island.building_front(Building::Well),
                        who: Who::You,
                    });
                    notes.write(Note::Feed(format!("Лейка полна: {} / {}", CAN_CAPACITY, CAN_CAPACITY)));
                }
                Building::Tent(w) => {
                    let text = if *w == Who::You {
                        "Твоя палатка. Тут ты отдыхаешь между визитами на остров".to_string()
                    } else {
                        format!("Палатка {}. Внутрь без спроса нельзя!", w.name_gen())
                    };
                    notes.write(Note::Feed(text));
                }
            },
            Interact::Sign(o) => {
                let text = match o.who() {
                    Some(Who::You) => "Твоя личная зона: строить и собирать тут можешь только ты. Друзья могут лишь полить".into(),
                    Some(w) => format!("Личная зона: {}. Можно поливать, но не трогать грядки и декор", w.name()),
                    None => "Общая зона: здесь все строят вместе".into(),
                };
                notes.write(Note::Toast(text, icon::COZY));
            }
            _ => continue,
        }
        if ui.menu.is_some() {
            play.write(Play::ui(S::Open).vol(0.5));
        }
    }
}

fn menu_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<UiState>,
    mut play: EventWriter<Play>,
    mut paused: ResMut<Paused>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        if ui.menu.is_some() {
            ui.menu = None;
            ui.dirty = true;
            play.write(Play::ui(S::Close).vol(0.5));
        } else {
            ui.open(Menu::Pause);
            play.write(Play::ui(S::Open).vol(0.5));
        }
    }
    if keys.just_pressed(KeyCode::Tab) {
        if ui.menu == Some(Menu::Chest) {
            ui.menu = None;
            ui.dirty = true;
            play.write(Play::ui(S::Close).vol(0.5));
        } else if ui.menu.is_none() {
            ui.open(Menu::Chest);
            play.write(Play::ui(S::Open).vol(0.5));
        }
    }
    if keys.just_pressed(KeyCode::KeyH) || keys.just_pressed(KeyCode::F1) {
        if ui.menu == Some(Menu::Help) {
            ui.menu = None;
            ui.dirty = true;
        } else if ui.menu.is_none() {
            ui.open(Menu::Help);
        }
    }
    if keys.just_pressed(KeyCode::KeyE) && ui.menu.is_some() && !matches!(ui.menu, Some(Menu::Intro) | Some(Menu::Welcome(_))) {
        // E toggles menus closed (opening is handled by the interact system next frame)
        ui.menu = None;
        ui.dirty = true;
        play.write(Play::ui(S::Close).vol(0.5));
    }
    let want = matches!(ui.menu, Some(Menu::Pause) | Some(Menu::Help) | Some(Menu::Intro) | Some(Menu::Welcome(_)));
    if paused.0 != want {
        paused.0 = want;
    }
}

fn title_row(p: &mut ChildSpawnerCommands, art: &Art, title: &str, sub: &str) {
    p.spawn(Node { flex_direction: FlexDirection::Column, margin: UiRect::bottom(Val::Px(8.0)), ..default() })
        .with_children(|c| {
            c.spawn((Text::new(title), font(art, 24.0), TextColor(INK)));
            if !sub.is_empty() {
                c.spawn((Text::new(sub), font(art, 14.0), TextColor(INK_SOFT)));
            }
        });
}

fn row() -> Node {
    Node {
        align_items: AlignItems::Center,
        column_gap: Val::Px(6.0),
        padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
        width: Val::Percent(100.0),
        ..default()
    }
}

fn spacer() -> Node {
    Node { flex_grow: 1.0, ..default() }
}

fn section(p: &mut ChildSpawnerCommands, art: &Art, s: &str) {
    p.spawn((Text::new(s), font(art, 16.0), TextColor(Color::srgb(0.55, 0.35, 0.2)), Node { margin: UiRect::top(Val::Px(6.0)), ..default() }));
}

#[allow(clippy::too_many_arguments)]
fn build_menu(
    mut commands: Commands,
    mut ui: ResMut<UiState>,
    art: Res<Art>,
    game: Res<Game>,
    clock: Res<SeasonClock>,
    settings: Res<AudioSettings>,
    roots: Query<Entity, With<MenuRoot>>,
    npcs: Query<(&crate::npc::Npc, &Visibility)>,
) {
    if !ui.dirty {
        return;
    }
    ui.dirty = false;
    for e in &roots {
        commands.entity(e).despawn();
    }
    let Some(menu) = ui.menu.clone() else { return };
    let art = &*art;
    let season = clock.season();
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.03, 0.1, 0.45)),
            GlobalZIndex(20),
            MenuRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(18.0)),
                    width: Val::Px(560.0),
                    max_height: Val::Percent(90.0),
                    overflow: Overflow::clip_y(),
                    ..default()
                },
                panel_img(art),
            ))
            .with_children(|p| {
                match &menu {
                    Menu::Kitchen => {
                        let bonus = if season.cooking_bonus() > 1.0 { " · зима: +50% опыта за блюда" } else { "" };
                        title_row(p, art, "Общая кухня", &format!("Блюда из общего урожая — дороже и дают опыт острову{}", bonus));
                        for d in Dish::ALL {
                            let unlocked = game.dish_unlocked(d);
                            let can = unlocked && d.recipe().iter().all(|(c, n)| game.count(Item::Crop(*c)) >= *n);
                            p.spawn(row()).with_children(|r| {
                                r.spawn(icon_node(art, d.icon(), 32.0));
                                r.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|c| {
                                    c.spawn((Text::new(d.name()), font(art, 16.0), TextColor(if unlocked { INK } else { INK_SOFT })));
                                    c.spawn((
                                        Text::new(format!("+{} опыта · цена {}", (d.xp() as f32 * season.cooking_bonus()) as u32, d.sell_price())),
                                        font(art, 12.0),
                                        TextColor(INK_SOFT),
                                    ));
                                });
                                r.spawn(spacer());
                                if unlocked {
                                    for (c, n) in d.recipe() {
                                        let have = game.count(Item::Crop(*c));
                                        r.spawn(icon_node(art, c.icon(), 24.0));
                                        r.spawn((Text::new(format!("{}/{}", have.min(99), n)), font(art, 14.0), TextColor(if have >= *n { GOOD } else { BAD })));
                                    }
                                    button(r, art, "Готовить", Act::Cook(d), can);
                                } else {
                                    r.spawn((Text::new(format!("откроется на ур. {}", d.unlock_level())), font(art, 13.0), TextColor(INK_SOFT)));
                                }
                            });
                        }
                    }
                    Menu::Shop => {
                        let disc = game.discount();
                        let sub = if disc < 1.0 { format!("Скидка декоратора {}%", ((1.0 - disc) * 100.0).round()) } else { "Лавка острова".into() };
                        title_row(p, art, "Лавка", &sub);
                        p.spawn(row()).with_children(|r| {
                            r.spawn(icon_node(art, icon::COIN, 24.0));
                            r.spawn((Text::new(format!("{} монет в общей казне", game.coins)), font(art, 15.0), TextColor(INK)));
                        });
                        section(p, art, "Семена");
                        for c in Crop::ALL {
                            let un = game.crop_unlocked(c);
                            let price = c.seed_price();
                            p.spawn(row()).with_children(|r| {
                                r.spawn(icon_node(art, c.seed_icon(), 32.0));
                                r.spawn((Text::new(format!("{} · есть {}", c.name(), game.count(Item::Seeds(c)))), font(art, 15.0), TextColor(if un { INK } else { INK_SOFT })));
                                r.spawn(spacer());
                                if un {
                                    r.spawn((Text::new(format!("{} за шт.", price)), font(art, 13.0), TextColor(INK_SOFT)));
                                    button(r, art, "×1", Act::BuySeeds(c, 1), game.coins >= price);
                                    button(r, art, "×5", Act::BuySeeds(c, 5), game.coins >= price * 5);
                                } else {
                                    r.spawn((Text::new(format!("ур. {}", c.unlock_level())), font(art, 13.0), TextColor(INK_SOFT)));
                                }
                            });
                        }
                        section(p, art, "Украшения");
                        for d in Decor::ALL {
                            let avail = game.decor_available(d, season);
                            if d.season().is_some_and(|s| s != season) || d.unlock_level() > game.level + 1 {
                                continue;
                            }
                            let price = game.price_of(d.price());
                            p.spawn(row()).with_children(|r| {
                                r.spawn((Node { width: Val::Px(32.0), height: Val::Px(32.0), ..default() }, ImageNode::new(art.decor(d))));
                                r.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|c| {
                                    c.spawn((Text::new(format!("{} · есть {}", d.name(), game.count(Item::Decor(d)))), font(art, 15.0), TextColor(if avail { INK } else { INK_SOFT })));
                                    let extra = if d.season().is_some() { " · сезонное, вернётся через год" } else { "" };
                                    c.spawn((Text::new(format!("уют +{}{}", d.cozy(), extra)), font(art, 12.0), TextColor(INK_SOFT)));
                                });
                                r.spawn(spacer());
                                if avail {
                                    r.spawn(icon_node(art, icon::COIN, 20.0));
                                    r.spawn((Text::new(format!("{}", price)), font(art, 14.0), TextColor(INK)));
                                    button(r, art, "Купить", Act::BuyDecor(d), game.coins >= price);
                                } else {
                                    r.spawn((Text::new(format!("ур. {}", d.unlock_level())), font(art, 13.0), TextColor(INK_SOFT)));
                                }
                            });
                        }
                    }
                    Menu::Sell => {
                        title_row(p, art, "Ящик продажи", "Монеты идут в общую казну. Блюда стоят дороже сырья!");
                        let items: Vec<(Item, u32)> = game
                            .storage
                            .iter()
                            .filter(|(it, _)| matches!(it, Item::Crop(_) | Item::Dish(_)))
                            .map(|(a, b)| (*a, *b))
                            .collect();
                        if items.is_empty() {
                            p.spawn((Text::new("Пока нечего продавать — собери урожай или приготовь блюдо"), font(art, 15.0), TextColor(INK_SOFT)));
                        }
                        for (it, n) in items {
                            p.spawn(row()).with_children(|r| {
                                let ic = match it {
                                    Item::Crop(c) => c.icon(),
                                    Item::Dish(d) => d.icon(),
                                    _ => 0,
                                };
                                r.spawn(icon_node(art, ic, 32.0));
                                r.spawn((Text::new(format!("{} ×{}", it.name(), n)), font(art, 15.0), TextColor(INK)));
                                r.spawn(spacer());
                                r.spawn(icon_node(art, icon::COIN, 20.0));
                                r.spawn((Text::new(format!("{}", it.sell_price())), font(art, 14.0), TextColor(INK)));
                                button(r, art, "Продать 1", Act::Sell(it, false), true);
                                button(r, art, "Все", Act::Sell(it, true), true);
                            });
                        }
                    }
                    Menu::Chest => {
                        title_row(p, art, "Общий склад", "Всё, что собрано и приготовлено всей компанией");
                        let mut any = false;
                        for (title, filter) in [("Урожай", 0u8), ("Блюда", 1), ("Семена", 2), ("Украшения", 3)] {
                            let items: Vec<(Item, u32)> = game
                                .storage
                                .iter()
                                .filter(|(it, _)| match (filter, it) {
                                    (0, Item::Crop(_)) | (1, Item::Dish(_)) | (2, Item::Seeds(_)) | (3, Item::Decor(_)) => true,
                                    _ => false,
                                })
                                .map(|(a, b)| (*a, *b))
                                .collect();
                            if items.is_empty() {
                                continue;
                            }
                            any = true;
                            section(p, art, title);
                            p.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(8.0), row_gap: Val::Px(6.0), ..default() })
                                .with_children(|g| {
                                    for (it, n) in items {
                                        let mut cell = g.spawn((
                                            Node {
                                                width: Val::Px(56.0),
                                                height: Val::Px(56.0),
                                                justify_content: JustifyContent::Center,
                                                align_items: AlignItems::Center,
                                                ..default()
                                            },
                                            ImageNode::new(art.ui_slot.clone()),
                                        ));
                                        if let Item::Decor(d) = it {
                                            cell.insert((Button, Act::SelectDecor(d)));
                                        }
                                        cell.with_children(|c| {
                                            match it {
                                                Item::Crop(cr) => {
                                                    c.spawn(icon_node(art, cr.icon(), 32.0));
                                                }
                                                Item::Dish(d) => {
                                                    c.spawn(icon_node(art, d.icon(), 32.0));
                                                }
                                                Item::Seeds(cr) => {
                                                    c.spawn(icon_node(art, cr.seed_icon(), 32.0));
                                                }
                                                Item::Decor(d) => {
                                                    c.spawn((Node { width: Val::Px(30.0), height: Val::Px(30.0), ..default() }, ImageNode::new(art.decor(d))));
                                                }
                                            }
                                            c.spawn((
                                                Text::new(format!("{}", n)),
                                                font(art, 13.0),
                                                TextColor(INK),
                                                Node { position_type: PositionType::Absolute, right: Val::Px(4.0), bottom: Val::Px(2.0), ..default() },
                                            ));
                                        });
                                    }
                                });
                        }
                        if !any {
                            p.spawn((Text::new("Склад пуст"), font(art, 15.0), TextColor(INK_SOFT)));
                        }
                        p.spawn((Text::new("Клик по украшению — выбрать его для слота [7]"), font(art, 12.0), TextColor(INK_SOFT), Node { margin: UiRect::top(Val::Px(8.0)), ..default() }));
                    }
                    Menu::Board => {
                        title_row(p, art, "Доска острова", &format!("Год {} · {} — {}", clock.year(), season.name(), season.bonus_text()));
                        section(p, art, "Общие цели");
                        for (i, g) in GOALS.iter().enumerate().skip(game.goal_idx.saturating_sub(1)).take(4) {
                            let (txt, col) = if i < game.goal_idx {
                                (format!("✓ {}", g.title).replace('✓', "+"), GOOD)
                            } else if i == game.goal_idx {
                                let (_, h, n) = game.goal_progress().unwrap();
                                (format!("> {} — {}/{}", g.title, h, n), INK)
                            } else {
                                (format!("  {}", g.title), INK_SOFT)
                            };
                            p.spawn((Text::new(txt), font(art, 14.0), TextColor(col)));
                        }
                        p.spawn((Text::new(format!("Выполнено целей: {} из {}", game.goal_idx, GOALS.len())), font(art, 12.0), TextColor(INK_SOFT)));
                        section(p, art, "Твои навыки");
                        for s in Skill::ALL {
                            let (x, need) = game.skill_progress(s);
                            p.spawn(row()).with_children(|r| {
                                r.spawn((Text::new(format!("{} {}", s.name(), game.skill(s))), font(art, 15.0), TextColor(INK), Node { width: Val::Px(130.0), ..default() }));
                                r.spawn((Node { width: Val::Px(90.0), height: Val::Px(7.0), ..default() }, BackgroundColor(Color::srgb(0.85, 0.78, 0.64))))
                                    .with_child((Node { width: Val::Percent(100.0 * x as f32 / need as f32), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgb(0.45, 0.6, 0.9))));
                                r.spawn((Text::new(s.perk(game.skill(s))), font(art, 12.0), TextColor(INK_SOFT)));
                            });
                        }
                        section(p, art, "Компания");
                        for (n, v) in &npcs {
                            let here = *v != Visibility::Hidden;
                            p.spawn((
                                Text::new(format!("{} ({}) — {}", n.who.name(), n.who.role(), if here { "на острове" } else { "отдыхает, скоро вернётся" })),
                                font(art, 14.0),
                                TextColor(if here { GOOD } else { INK_SOFT }),
                            ));
                        }
                        p.spawn((Text::new(format!("Питомцы: {} · Уют: {}", game.pets.len(), game.cozy)), font(art, 14.0), TextColor(INK)));
                    }
                    Menu::Pause => {
                        title_row(p, art, "Пауза", "Ничего не завянет, пока тебя нет. Возвращайся когда угодно");
                        p.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), align_items: AlignItems::Start, ..default() })
                            .with_children(|c| {
                                button(c, art, "Продолжить", Act::Close, true);
                                button(c, art, if settings.sfx { "Звуки: вкл" } else { "Звуки: выкл" }, Act::ToggleSfx, true);
                                button(c, art, if settings.music { "Музыка: вкл" } else { "Музыка: выкл" }, Act::ToggleMusic, true);
                                button(c, art, "Управление", Act::Help, true);
                                button(c, art, "Сохранить и выйти", Act::SaveQuit, true);
                            });
                    }
                    Menu::Help | Menu::Intro => {
                        if menu == Menu::Intro {
                            title_row(p, art, "Добро пожаловать в The Garden!", "Общий остров для тебя и друзей. Бета-версия");
                            p.spawn((
                                Text::new(
                                    "Сажайте, поливайте, собирайте и готовьте вместе — прогресс общий.\n\
                                     У каждого есть личная зона (только хозяин строит), а в центре — общая.\n\
                                     Растения не вянут и ничего не сгорит, пока тебя нет.\n\
                                     Друзья Мила, Тёма и Ксю заглядывают на остров, когда им удобно.",
                                ),
                                font(art, 15.0),
                                TextColor(INK),
                                Node { margin: UiRect::bottom(Val::Px(8.0)), ..default() },
                            ));
                        } else {
                            title_row(p, art, "Управление", "");
                        }
                        for (k, v) in [
                            ("WASD / стрелки", "ходить (Shift — быстрее)"),
                            ("ЛКМ / Пробел", "действие инструментом (можно зажать)"),
                            ("ПКМ", "убрать украшение"),
                            ("1–7 / колесо", "выбрать слот"),
                            ("R", "сменить украшение в слоте 7"),
                            ("E", "кухня, лавка, ящик, друзья, питомцы"),
                            ("Tab", "общий склад"),
                            ("+ / −", "приближение камеры"),
                            ("Esc", "пауза и настройки"),
                        ] {
                            p.spawn(row()).with_children(|r| {
                                r.spawn((Text::new(k), font(art, 15.0), TextColor(Color::srgb(0.55, 0.35, 0.2)), Node { width: Val::Px(160.0), ..default() }));
                                r.spawn((Text::new(v), font(art, 15.0), TextColor(INK)));
                            });
                        }
                        p.spawn(Node { margin: UiRect::top(Val::Px(10.0)), ..default() })
                            .with_children(|c| button(c, art, if menu == Menu::Intro { "Начать!" } else { "Понятно" }, Act::Close, true));
                    }
                    Menu::Welcome(lines) => {
                        title_row(p, art, "С возвращением!", "Пока тебя не было, остров жил своей жизнью");
                        for l in lines {
                            p.spawn((Text::new(format!("• {}", l)), font(art, 15.0), TextColor(INK)));
                        }
                        p.spawn(Node { margin: UiRect::top(Val::Px(10.0)), ..default() })
                            .with_children(|c| button(c, art, "Отлично!", Act::Close, true));
                    }
                }
                if !matches!(menu, Menu::Pause | Menu::Help | Menu::Intro | Menu::Welcome(_)) {
                    p.spawn(Node { margin: UiRect::top(Val::Px(10.0)), justify_content: JustifyContent::End, ..default() })
                        .with_children(|c| {
                            c.spawn((Text::new("Esc / E — закрыть"), font(art, 12.0), TextColor(INK_SOFT), Node { margin: UiRect::right(Val::Px(8.0)), align_self: AlignSelf::Center, ..default() }));
                            button(c, art, "Закрыть", Act::Close, true);
                        });
                }
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn menu_buttons(
    mut commands: Commands,
    q: Query<(&Interaction, &Act), Changed<Interaction>>,
    mut ui: ResMut<UiState>,
    mut game: ResMut<Game>,
    mut settings: ResMut<AudioSettings>,
    mut play: EventWriter<Play>,
    mut notes: EventWriter<Note>,
    mut hb: ResMut<Hotbar>,
    clock: Res<SeasonClock>,
    mut exit: EventWriter<AppExit>,
    mut save: EventWriter<crate::save::SaveNow>,
) {
    let mut rng = rand::thread_rng();
    for (i, act) in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        ui.dirty = true;
        match act.clone() {
            Act::Close => {
                ui.menu = None;
                play.write(Play::ui(S::Close).vol(0.5));
            }
            Act::Cook(d) => {
                if game.dish_unlocked(d) && d.recipe().iter().all(|(c, n)| game.count(Item::Crop(*c)) >= *n) {
                    for (c, n) in d.recipe() {
                        game.take(Item::Crop(*c), *n);
                    }
                    let lvl = game.skill(Skill::Cook);
                    let double = rng.gen::<f32>() < 0.05 * lvl as f32;
                    let n = if double { 2 } else { 1 };
                    game.add(Item::Dish(d), n);
                    *game.stats.cooked.entry(d).or_insert(0) += n;
                    game.stats.cooked_total += n;
                    let xp = (d.xp() as f32 * clock.season().cooking_bonus()) as u32;
                    game.add_xp(xp, &mut notes);
                    game.add_skill_xp(Skill::Cook, d.xp(), &mut notes);
                    if game.tutorial == 4 {
                        game.tutorial = 5;
                    }
                    play.write(Play::ui(S::Cook).vol(0.6));
                    notes.write(Note::Toast(
                        format!("Готово: {}{} · +{} опыта острову", d.name(), if double { " ×2 (талант повара!)" } else { "" }, xp),
                        d.icon(),
                    ));
                    commands.spawn(ChimneySmoke(2.5));
                } else {
                    play.write(Play::ui(S::Deny).vol(0.4));
                }
            }
            Act::BuySeeds(c, n) => {
                let price = c.seed_price() * n;
                if game.coins >= price && game.crop_unlocked(c) {
                    game.coins -= price;
                    game.add(Item::Seeds(c), n);
                    play.write(Play::ui(S::Coin).vol(0.5));
                } else {
                    play.write(Play::ui(S::Deny).vol(0.4));
                }
            }
            Act::BuyDecor(d) => {
                let price = game.price_of(d.price());
                if game.coins >= price && game.decor_available(d, clock.season()) {
                    game.coins -= price;
                    game.add(Item::Decor(d), 1);
                    hb.decor = d;
                    hb.sel = 6;
                    hb.changed_t = 0.0;
                    play.write(Play::ui(S::Coin).vol(0.5));
                    notes.write(Note::Feed(format!("Куплено: {} — выбрано в слоте [7]", d.name())));
                } else {
                    play.write(Play::ui(S::Deny).vol(0.4));
                }
            }
            Act::Sell(it, all) => {
                let n = if all { game.count(it) } else { 1.min(game.count(it)) };
                if n > 0 && game.take(it, n) {
                    let coins = it.sell_price() * n;
                    game.coins += coins;
                    game.stats.sold += coins;
                    if game.tutorial == 4 {
                        game.tutorial = 5;
                    }
                    play.write(Play::ui(S::Coin).vol(0.55).pitch(if all { 1.0 } else { 1.1 }));
                    notes.write(Note::Feed(format!("Продано: {} ×{} — +{} монет", it.name(), n, coins)));
                }
            }
            Act::SelectDecor(d) => {
                hb.decor = d;
                hb.sel = 6;
                hb.changed_t = 0.0;
                ui.menu = None;
                play.write(Play::ui(S::Select));
            }
            Act::ToggleSfx => {
                settings.sfx = !settings.sfx;
                play.write(Play::ui(S::Click));
            }
            Act::ToggleMusic => {
                settings.music = !settings.music;
                play.write(Play::ui(S::Click));
            }
            Act::Help => {
                ui.menu = Some(Menu::Help);
                play.write(Play::ui(S::Click));
            }
            Act::SaveQuit => {
                save.write(crate::save::SaveNow);
                exit.write(AppExit::Success);
            }
        }
    }
}

fn button_hover(mut q: Query<(&Interaction, &mut ImageNode), (Changed<Interaction>, With<Button>)>, mut play: EventWriter<Play>) {
    for (i, mut img) in &mut q {
        match i {
            Interaction::Hovered => {
                img.color = Color::srgb(1.12, 1.12, 1.05);
                play.write(Play::ui(S::Click).vol(0.12).pitch(1.4));
            }
            Interaction::Pressed => img.color = Color::srgb(0.85, 0.85, 0.85),
            Interaction::None => img.color = Color::WHITE,
        }
    }
}

fn pointer_over(
    q: Query<(&Interaction, Option<&HotSlot>), With<Button>>,
    mut over: ResMut<PointerOverUi>,
    mut hb: ResMut<Hotbar>,
    mut play: EventWriter<Play>,
) {
    let mut any = false;
    for (i, slot) in &q {
        if *i != Interaction::None {
            any = true;
        }
        if let (Interaction::Pressed, Some(s)) = (i, slot) {
            if hb.sel != s.0 {
                hb.sel = s.0;
                hb.changed_t = 0.0;
                play.write(Play::ui(S::Select).vol(0.4));
            }
        }
    }
    over.0 = any;
}

fn smoke(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut ChimneySmoke)>,
    chimney: Query<&Transform, With<Chimney>>,
    mut acc: Local<f32>,
) {
    let Ok(ct) = chimney.single() else { return };
    let mut rng = rand::thread_rng();
    *acc += time.delta_secs();
    let mut active = false;
    for (e, mut s) in &mut q {
        s.0 -= time.delta_secs();
        active = true;
        if s.0 <= 0.0 {
            commands.entity(e).despawn();
        }
    }
    // a lazy wisp even when nobody cooks
    let interval = if active { 0.06 } else { 0.7 };
    if *acc >= interval {
        *acc = 0.0;
        let g = rng.gen_range(0.82..0.95);
        let _ = spawn_particle(
            &mut commands,
            ct.translation.truncate() + Vec2::new(rng.gen_range(-2.0..2.0), 0.0),
            30.0,
            Vec2::new(rng.gen_range(4.0..12.0), rng.gen_range(14.0..24.0)),
            Color::srgba(g, g, g, if active { 0.8 } else { 0.45 }),
            rng.gen_range(3.0..5.0),
            rng.gen_range(1.2..2.0),
            -4.0,
        );
    }
}
