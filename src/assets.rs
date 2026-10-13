//! All assets are embedded into the executable, so the build is a single
//! self-contained file that runs from anywhere.

use bevy::asset::RenderAssetUsages;
use bevy::audio::AudioSource;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use std::sync::Arc;

use crate::data::{icon, Decor, Season, Who};

macro_rules! sprite {
    ($name:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sprites/", $name))
    };
}
macro_rules! sound {
    ($name:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/audio/", $name))
    };
}

pub const MAP_TXT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/map/island.txt"));

#[derive(Resource)]
pub struct Art {
    pub font: Handle<Font>,
    pub terrain: [Handle<Image>; 4],
    pub foam: [Handle<Image>; 3],
    pub water: Handle<Image>,
    pub water_layout: Handle<TextureAtlasLayout>,
    pub chars: Vec<(Who, Handle<Image>)>,
    pub char_layout: Handle<TextureAtlasLayout>,
    pub crops: Handle<Image>,
    pub crops_layout: Handle<TextureAtlasLayout>,
    pub soil: Handle<Image>,
    pub soil_layout: Handle<TextureAtlasLayout>,
    pub icons: Handle<Image>,
    pub icons_layout: Handle<TextureAtlasLayout>,
    pub trees: [Handle<Image>; 4],
    pub trees_layout: Handle<TextureAtlasLayout>,
    pub bush: [Handle<Image>; 4],
    pub rock: Handle<Image>,
    pub kitchen: Handle<Image>,
    pub kitchen_layout: Handle<TextureAtlasLayout>,
    pub well: Handle<Image>,
    pub shop: Handle<Image>,
    pub chest: Handle<Image>,
    pub crate_box: Handle<Image>,
    pub board: Handle<Image>,
    pub tents: Vec<(Who, Handle<Image>)>,
    pub sign: Handle<Image>,
    pub dock: Handle<Image>,
    pub boat: Handle<Image>,
    pub decor: Vec<(Decor, Handle<Image>)>,
    pub fence_tiles: Handle<Image>,
    pub fence_layout: Handle<TextureAtlasLayout>,
    pub cat: Handle<Image>,
    pub fox: Handle<Image>,
    pub pet_layout: Handle<TextureAtlasLayout>,
    pub butterfly: Handle<Image>,
    pub butterfly_layout: Handle<TextureAtlasLayout>,
    pub light: Handle<Image>,
    pub shadow: Handle<Image>,
    pub cloud: Handle<Image>,
    pub cursor: Handle<Image>,
    pub ui_panel: Handle<Image>,
    pub ui_dark: Handle<Image>,
    pub ui_slot: Handle<Image>,
    pub ui_slot_sel: Handle<Image>,
    pub ui_bubble: Handle<Image>,
    pub ui_button: Handle<Image>,
    pub ui_button_off: Handle<Image>,
    pub logo: Handle<Image>,
}

impl Art {
    pub fn char(&self, who: Who) -> Handle<Image> {
        self.chars.iter().find(|(w, _)| *w == who).unwrap().1.clone()
    }
    pub fn tent(&self, who: Who) -> Handle<Image> {
        self.tents.iter().find(|(w, _)| *w == who).unwrap().1.clone()
    }
    pub fn decor(&self, d: Decor) -> Handle<Image> {
        self.decor.iter().find(|(k, _)| *k == d).unwrap().1.clone()
    }
    pub fn icon(&self, idx: usize) -> Sprite {
        Sprite::from_atlas_image(
            self.icons.clone(),
            TextureAtlas { layout: self.icons_layout.clone(), index: idx },
        )
    }
    pub fn icon_node(&self, idx: usize) -> ImageNode {
        ImageNode::from_atlas_image(
            self.icons.clone(),
            TextureAtlas { layout: self.icons_layout.clone(), index: idx },
        )
    }
    pub fn trees_for(&self, s: Season) -> Handle<Image> {
        self.trees[s.idx()].clone()
    }
}

#[derive(Resource)]
pub struct Sfx {
    pub step: [Handle<AudioSource>; 2],
    pub hoe: Handle<AudioSource>,
    pub water: Handle<AudioSource>,
    pub plant: Handle<AudioSource>,
    pub harvest: Handle<AudioSource>,
    pub coin: Handle<AudioSource>,
    pub pickup: Handle<AudioSource>,
    pub click: Handle<AudioSource>,
    pub open: Handle<AudioSource>,
    pub close: Handle<AudioSource>,
    pub select: Handle<AudioSource>,
    pub deny: Handle<AudioSource>,
    pub cook: Handle<AudioSource>,
    pub levelup: Handle<AudioSource>,
    pub quest: Handle<AudioSource>,
    pub meow: Handle<AudioSource>,
    pub yip: Handle<AudioSource>,
    pub purr: Handle<AudioSource>,
    pub splash: Handle<AudioSource>,
    pub place: Handle<AudioSource>,
    pub horn: Handle<AudioSource>,
    pub birds: [Handle<AudioSource>; 3],
    pub night: Handle<AudioSource>,
    pub waves: Handle<AudioSource>,
    pub music: Handle<AudioSource>,
}

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_assets);
    }
}

fn load_assets(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut fonts: ResMut<Assets<Font>>,
    mut audio: ResMut<Assets<AudioSource>>,
) {
    let mut img = |bytes: &'static [u8]| -> Handle<Image> {
        let image = Image::from_buffer(
            bytes,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::nearest(),
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        )
        .expect("embedded png");
        images.add(image)
    };
    let mut grid = |w: u32, h: u32, cols: u32, rows: u32| -> Handle<TextureAtlasLayout> {
        layouts.add(TextureAtlasLayout::from_grid(UVec2::new(w, h), cols, rows, None, None))
    };
    let font = fonts.add(
        Font::try_from_bytes(
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/Nunito-Garden.ttf"))
                .to_vec(),
        )
        .expect("font"),
    );
    let water_layout = grid(32, 32, 4, 1);
    let char_layout = grid(18, 26, 5, 3);
    let crops_layout = grid(16, 24, 4, 4);
    let soil_layout = grid(16, 16, 2, 1);
    let icons_layout = grid(16, 16, icon::COUNT as u32, 1);
    let trees_layout = grid(32, 44, 2, 1);
    let kitchen_layout = grid(72, 64, 2, 1);
    let pet_layout = grid(16, 16, 6, 1);
    let butterfly_layout = grid(8, 8, 8, 1);
    let fence_layout = grid(16, 16, 16, 1);

    let art = Art {
        font,
        terrain: [
            img(sprite!("terrain_spring.png")),
            img(sprite!("terrain_summer.png")),
            img(sprite!("terrain_autumn.png")),
            img(sprite!("terrain_winter.png")),
        ],
        foam: [img(sprite!("foam_0.png")), img(sprite!("foam_1.png")), img(sprite!("foam_2.png"))],
        water: img(sprite!("water.png")),
        water_layout,
        chars: vec![
            (Who::You, img(sprite!("char_player.png"))),
            (Who::Mila, img(sprite!("char_mila.png"))),
            (Who::Tyoma, img(sprite!("char_tyoma.png"))),
            (Who::Ksyu, img(sprite!("char_ksyu.png"))),
        ],
        char_layout,
        crops: img(sprite!("crops.png")),
        crops_layout,
        soil: img(sprite!("soil.png")),
        soil_layout,
        icons: img(sprite!("icons.png")),
        icons_layout,
        trees: [
            img(sprite!("trees_spring.png")),
            img(sprite!("trees_summer.png")),
            img(sprite!("trees_autumn.png")),
            img(sprite!("trees_winter.png")),
        ],
        trees_layout,
        bush: [
            img(sprite!("bush_spring.png")),
            img(sprite!("bush_summer.png")),
            img(sprite!("bush_autumn.png")),
            img(sprite!("bush_winter.png")),
        ],
        rock: img(sprite!("rock.png")),
        kitchen: img(sprite!("kitchen.png")),
        kitchen_layout,
        well: img(sprite!("well.png")),
        shop: img(sprite!("shop.png")),
        chest: img(sprite!("chest.png")),
        crate_box: img(sprite!("crate.png")),
        board: img(sprite!("board.png")),
        tents: vec![
            (Who::You, img(sprite!("tent_you.png"))),
            (Who::Mila, img(sprite!("tent_mila.png"))),
            (Who::Tyoma, img(sprite!("tent_tyoma.png"))),
            (Who::Ksyu, img(sprite!("tent_ksyu.png"))),
        ],
        sign: img(sprite!("sign.png")),
        dock: img(sprite!("dock.png")),
        boat: img(sprite!("boat.png")),
        decor: vec![
            (Decor::Fence, img(sprite!("decor_fence.png"))),
            (Decor::Flowerbed, img(sprite!("decor_flowerbed.png"))),
            (Decor::Lantern, img(sprite!("decor_lantern.png"))),
            (Decor::Bowl, img(sprite!("decor_bowl.png"))),
            (Decor::Bench, img(sprite!("decor_bench.png"))),
            (Decor::Scarecrow, img(sprite!("decor_scarecrow.png"))),
            (Decor::Birdhouse, img(sprite!("decor_birdhouse.png"))),
            (Decor::Mushroom, img(sprite!("decor_mushroom.png"))),
            (Decor::Sunflower, img(sprite!("decor_sunflower.png"))),
            (Decor::Pumpkin, img(sprite!("decor_pumpkin.png"))),
            (Decor::Snowman, img(sprite!("decor_snowman.png"))),
        ],
        fence_tiles: img(sprite!("fence_tiles.png")),
        fence_layout,
        cat: img(sprite!("pet_cat.png")),
        fox: img(sprite!("pet_fox.png")),
        pet_layout,
        butterfly: img(sprite!("butterfly.png")),
        butterfly_layout,
        light: img(sprite!("light.png")),
        shadow: img(sprite!("shadow.png")),
        cloud: img(sprite!("cloud.png")),
        cursor: img(sprite!("cursor.png")),
        ui_panel: img(sprite!("ui_panel.png")),
        ui_dark: img(sprite!("ui_dark.png")),
        ui_slot: img(sprite!("ui_slot.png")),
        ui_slot_sel: img(sprite!("ui_slot_sel.png")),
        ui_bubble: img(sprite!("ui_bubble.png")),
        ui_button: img(sprite!("ui_button.png")),
        ui_button_off: img(sprite!("ui_button_off.png")),
        logo: img(sprite!("logo.png")),
    };
    commands.insert_resource(art);

    let mut snd = |bytes: &'static [u8]| audio.add(AudioSource { bytes: Arc::from(bytes) });
    commands.insert_resource(Sfx {
        step: [snd(sound!("step0.wav")), snd(sound!("step1.wav"))],
        hoe: snd(sound!("hoe.wav")),
        water: snd(sound!("water.wav")),
        plant: snd(sound!("plant.wav")),
        harvest: snd(sound!("harvest.wav")),
        coin: snd(sound!("coin.wav")),
        pickup: snd(sound!("pickup.wav")),
        click: snd(sound!("click.wav")),
        open: snd(sound!("open.wav")),
        close: snd(sound!("close.wav")),
        select: snd(sound!("select.wav")),
        deny: snd(sound!("deny.wav")),
        cook: snd(sound!("cook.wav")),
        levelup: snd(sound!("levelup.wav")),
        quest: snd(sound!("quest.wav")),
        meow: snd(sound!("meow.wav")),
        yip: snd(sound!("yip.wav")),
        purr: snd(sound!("purr.wav")),
        splash: snd(sound!("splash.wav")),
        place: snd(sound!("place.wav")),
        horn: snd(sound!("horn.wav")),
        birds: [snd(sound!("bird0.wav")), snd(sound!("bird1.wav")), snd(sound!("bird2.wav"))],
        night: snd(sound!("night.wav")),
        waves: snd(sound!("waves.wav")),
        music: snd(sound!("music.wav")),
    });
}
