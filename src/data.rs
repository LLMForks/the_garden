//! Static game data: crops, dishes, decor, items, unlocks and goals.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Crop {
    Wheat,
    Carrot,
    Cabbage,
    Potato,
}

impl Crop {
    pub const ALL: [Crop; 4] = [Crop::Wheat, Crop::Carrot, Crop::Cabbage, Crop::Potato];

    pub fn name(self) -> &'static str {
        match self {
            Crop::Wheat => "Пшеница",
            Crop::Carrot => "Морковь",
            Crop::Cabbage => "Капуста",
            Crop::Potato => "Картофель",
        }
    }
    pub fn name_gen(self) -> &'static str {
        match self {
            Crop::Wheat => "пшеницу",
            Crop::Carrot => "морковь",
            Crop::Cabbage => "капусту",
            Crop::Potato => "картофель",
        }
    }
    /// Base growth time in seconds without water.
    pub fn grow_secs(self) -> f32 {
        match self {
            Crop::Wheat => 110.0,
            Crop::Carrot => 140.0,
            Crop::Cabbage => 190.0,
            Crop::Potato => 230.0,
        }
    }
    pub fn sell_price(self) -> u32 {
        match self {
            Crop::Wheat => 4,
            Crop::Carrot => 6,
            Crop::Cabbage => 9,
            Crop::Potato => 8,
        }
    }
    pub fn seed_price(self) -> u32 {
        match self {
            Crop::Wheat => 2,
            Crop::Carrot => 3,
            Crop::Cabbage => 5,
            Crop::Potato => 4,
        }
    }
    pub fn unlock_level(self) -> u32 {
        match self {
            Crop::Wheat | Crop::Carrot => 1,
            Crop::Cabbage => 2,
            Crop::Potato => 3,
        }
    }
    /// row in crops.png
    pub fn sprite_row(self) -> usize {
        match self {
            Crop::Wheat => 0,
            Crop::Cabbage => 1,
            Crop::Carrot => 2,
            Crop::Potato => 3,
        }
    }
    pub fn icon(self) -> usize {
        match self {
            Crop::Wheat => icon::CROP,
            Crop::Cabbage => icon::CROP + 1,
            Crop::Carrot => icon::CROP + 2,
            Crop::Potato => icon::CROP + 3,
        }
    }
    pub fn seed_icon(self) -> usize {
        self.icon() - 4
    }
    pub fn particle_color(self) -> [f32; 3] {
        match self {
            Crop::Wheat => [0.95, 0.80, 0.35],
            Crop::Carrot => [0.95, 0.55, 0.18],
            Crop::Cabbage => [0.60, 0.85, 0.45],
            Crop::Potato => [0.80, 0.62, 0.38],
        }
    }
}

/// Indices into icons.png (16x16 strip).
pub mod icon {
    pub const HOE: usize = 0;
    pub const CAN: usize = 1;
    // seeds: wheat, cabbage, carrot, potato (2..=5)
    pub const CROP: usize = 6; // wheat, cabbage, carrot, potato (6..=9)
    pub const DISH: usize = 10; // 6 dishes (10..=15)
    pub const COIN: usize = 16;
    pub const HEART: usize = 17;
    pub const DROP: usize = 18;
    pub const STAR: usize = 19;
    pub const COZY: usize = 20;
    pub const SPARKLE: usize = 21;
    pub const COUNT: usize = 22;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Dish {
    Bread,
    Salad,
    Mash,
    Stew,
    CarrotPie,
    CabbagePie,
}

impl Dish {
    pub const ALL: [Dish; 6] = [
        Dish::Bread,
        Dish::Salad,
        Dish::Mash,
        Dish::Stew,
        Dish::CarrotPie,
        Dish::CabbagePie,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Dish::Bread => "Хлеб",
            Dish::Salad => "Салат",
            Dish::Mash => "Пюре",
            Dish::Stew => "Овощное рагу",
            Dish::CarrotPie => "Морковный пирог",
            Dish::CabbagePie => "Пирог с капустой",
        }
    }
    pub fn recipe(self) -> &'static [(Crop, u32)] {
        match self {
            Dish::Bread => &[(Crop::Wheat, 3)],
            Dish::Salad => &[(Crop::Cabbage, 1), (Crop::Carrot, 1)],
            Dish::Mash => &[(Crop::Potato, 2)],
            Dish::Stew => &[(Crop::Potato, 1), (Crop::Carrot, 1), (Crop::Cabbage, 1)],
            Dish::CarrotPie => &[(Crop::Wheat, 2), (Crop::Carrot, 2)],
            Dish::CabbagePie => &[(Crop::Wheat, 2), (Crop::Cabbage, 1)],
        }
    }
    pub fn sell_price(self) -> u32 {
        match self {
            Dish::Bread => 18,
            Dish::Salad => 22,
            Dish::Mash => 22,
            Dish::Stew => 34,
            Dish::CarrotPie => 32,
            Dish::CabbagePie => 30,
        }
    }
    pub fn xp(self) -> u32 {
        match self {
            Dish::Bread => 14,
            Dish::Salad => 16,
            Dish::Mash => 16,
            Dish::Stew => 26,
            Dish::CarrotPie => 24,
            Dish::CabbagePie => 22,
        }
    }
    pub fn unlock_level(self) -> u32 {
        match self {
            Dish::Bread => 1,
            Dish::Salad => 2,
            Dish::Mash | Dish::Stew => 3,
            Dish::CarrotPie | Dish::CabbagePie => 4,
        }
    }
    pub fn icon(self) -> usize {
        icon::DISH + Dish::ALL.iter().position(|d| *d == self).unwrap()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub const ALL: [Season; 4] = [Season::Spring, Season::Summer, Season::Autumn, Season::Winter];
    pub fn name(self) -> &'static str {
        match self {
            Season::Spring => "Весна",
            Season::Summer => "Лето",
            Season::Autumn => "Осень",
            Season::Winter => "Зима",
        }
    }
    pub fn idx(self) -> usize {
        self as usize
    }
    pub fn growth_bonus(self, crop: Crop) -> f32 {
        match (self, crop) {
            (Season::Spring, Crop::Carrot) => 1.5,
            (Season::Spring, Crop::Cabbage) => 1.25,
            (Season::Summer, Crop::Wheat) => 1.5,
            (Season::Summer, Crop::Carrot) => 1.2,
            (Season::Autumn, Crop::Potato) => 1.5,
            (Season::Autumn, Crop::Cabbage) => 1.25,
            _ => 1.0,
        }
    }
    pub fn cooking_bonus(self) -> f32 {
        if self == Season::Winter {
            1.5
        } else {
            1.0
        }
    }
    pub fn bonus_text(self) -> &'static str {
        match self {
            Season::Spring => "Морковь и капуста растут быстрее",
            Season::Summer => "Пшеница растёт быстрее",
            Season::Autumn => "Картофель и капуста растут быстрее",
            Season::Winter => "Время уюта: блюда дают +50% опыта",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Decor {
    Fence,
    Flowerbed,
    Lantern,
    Bowl,
    Bench,
    Scarecrow,
    Birdhouse,
    Mushroom,
    Sunflower,
    Pumpkin,
    Snowman,
}

impl Decor {
    pub const ALL: [Decor; 11] = [
        Decor::Fence,
        Decor::Flowerbed,
        Decor::Lantern,
        Decor::Bowl,
        Decor::Bench,
        Decor::Scarecrow,
        Decor::Birdhouse,
        Decor::Mushroom,
        Decor::Sunflower,
        Decor::Pumpkin,
        Decor::Snowman,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Decor::Fence => "Заборчик",
            Decor::Flowerbed => "Клумба",
            Decor::Lantern => "Фонарь",
            Decor::Bowl => "Миска для питомцев",
            Decor::Bench => "Скамейка",
            Decor::Scarecrow => "Пугало",
            Decor::Birdhouse => "Скворечник",
            Decor::Mushroom => "Грибной светильник",
            Decor::Sunflower => "Подсолнух",
            Decor::Pumpkin => "Тыква-фонарь",
            Decor::Snowman => "Снеговик",
        }
    }
    pub fn file(self) -> &'static str {
        match self {
            Decor::Fence => "fence",
            Decor::Flowerbed => "flowerbed",
            Decor::Lantern => "lantern",
            Decor::Bowl => "bowl",
            Decor::Bench => "bench",
            Decor::Scarecrow => "scarecrow",
            Decor::Birdhouse => "birdhouse",
            Decor::Mushroom => "mushroom",
            Decor::Sunflower => "sunflower",
            Decor::Pumpkin => "pumpkin",
            Decor::Snowman => "snowman",
        }
    }
    pub fn price(self) -> u32 {
        match self {
            Decor::Fence => 8,
            Decor::Flowerbed => 20,
            Decor::Lantern => 30,
            Decor::Bowl => 25,
            Decor::Bench => 45,
            Decor::Scarecrow => 35,
            Decor::Birdhouse => 40,
            Decor::Mushroom => 40,
            Decor::Sunflower => 30,
            Decor::Pumpkin => 40,
            Decor::Snowman => 40,
        }
    }
    pub fn cozy(self) -> u32 {
        match self {
            Decor::Fence => 1,
            Decor::Flowerbed => 2,
            Decor::Lantern => 2,
            Decor::Bowl => 2,
            Decor::Bench => 3,
            Decor::Scarecrow => 2,
            Decor::Birdhouse => 3,
            Decor::Mushroom => 3,
            Decor::Sunflower => 3,
            Decor::Pumpkin => 4,
            Decor::Snowman => 4,
        }
    }
    pub fn unlock_level(self) -> u32 {
        match self {
            Decor::Fence | Decor::Flowerbed => 1,
            Decor::Lantern | Decor::Bowl => 2,
            Decor::Bench | Decor::Scarecrow => 3,
            Decor::Birdhouse | Decor::Mushroom => 4,
            Decor::Sunflower | Decor::Pumpkin | Decor::Snowman => 2,
        }
    }
    /// Seasonal decor is only sold in its season, but comes back every year.
    pub fn season(self) -> Option<Season> {
        match self {
            Decor::Sunflower => Some(Season::Summer),
            Decor::Pumpkin => Some(Season::Autumn),
            Decor::Snowman => Some(Season::Winter),
            Decor::Birdhouse => None,
            _ => None,
        }
    }
    pub fn glows(self) -> bool {
        matches!(self, Decor::Lantern | Decor::Mushroom | Decor::Pumpkin)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Item {
    Seeds(Crop),
    Crop(Crop),
    Dish(Dish),
    Decor(Decor),
}

impl Item {
    pub fn name(self) -> String {
        match self {
            Item::Seeds(c) => format!("Семена: {}", c.name().to_lowercase()),
            Item::Crop(c) => c.name().to_string(),
            Item::Dish(d) => d.name().to_string(),
            Item::Decor(d) => d.name().to_string(),
        }
    }
    pub fn sell_price(self) -> u32 {
        match self {
            Item::Seeds(c) => c.seed_price() / 2,
            Item::Crop(c) => c.sell_price(),
            Item::Dish(d) => d.sell_price(),
            Item::Decor(d) => d.price() / 2,
        }
    }
}

/// Who did something on the island.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Who {
    You,
    Mila,
    Tyoma,
    Ksyu,
}

impl Who {
    pub const FRIENDS: [Who; 3] = [Who::Mila, Who::Tyoma, Who::Ksyu];
    pub fn name(self) -> &'static str {
        match self {
            Who::You => "Ты",
            Who::Mila => "Мила",
            Who::Tyoma => "Тёма",
            Who::Ksyu => "Ксю",
        }
    }
    pub fn name_gen(self) -> &'static str {
        match self {
            Who::You => "твоя",
            Who::Mila => "Милы",
            Who::Tyoma => "Тёмы",
            Who::Ksyu => "Ксю",
        }
    }
    pub fn role(self) -> &'static str {
        match self {
            Who::You => "фермер",
            Who::Mila => "садовница",
            Who::Tyoma => "повар",
            Who::Ksyu => "декоратор",
        }
    }
    pub fn zone(self) -> Owner {
        match self {
            Who::You => Owner::You,
            Who::Mila => Owner::Mila,
            Who::Tyoma => Owner::Tyoma,
            Who::Ksyu => Owner::Ksyu,
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Who::You => [0.31, 0.70, 0.66],
            Who::Mila => [0.49, 0.75, 0.35],
            Who::Tyoma => [0.91, 0.42, 0.35],
            Who::Ksyu => [0.69, 0.54, 0.88],
        }
    }
    pub fn file(self) -> &'static str {
        match self {
            Who::You => "player",
            Who::Mila => "mila",
            Who::Tyoma => "tyoma",
            Who::Ksyu => "ksyu",
        }
    }
}

/// Ownership of a tile on the island.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Owner {
    /// The shared zone and the wild parts of the island: everyone may build here.
    Shared,
    You,
    Mila,
    Tyoma,
    Ksyu,
}

impl Owner {
    pub fn who(self) -> Option<Who> {
        match self {
            Owner::Shared => None,
            Owner::You => Some(Who::You),
            Owner::Mila => Some(Who::Mila),
            Owner::Tyoma => Some(Who::Tyoma),
            Owner::Ksyu => Some(Who::Ksyu),
        }
    }
    pub fn can_build(self, who: Who) -> bool {
        match self.who() {
            None => true,
            Some(o) => o == who,
        }
    }
}

/// Island level progression: xp needed to reach `level + 1`.
pub fn xp_for_next(level: u32) -> u32 {
    match level {
        1 => 60,
        2 => 140,
        3 => 240,
        4 => 380,
        5 => 560,
        l => 560 + (l - 5) * 220,
    }
}

pub fn skill_xp_for_next(level: u32) -> u32 {
    20 + level * 18
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Skill {
    Gardener,
    Cook,
    Decorator,
}

impl Skill {
    pub const ALL: [Skill; 3] = [Skill::Gardener, Skill::Cook, Skill::Decorator];
    pub fn name(self) -> &'static str {
        match self {
            Skill::Gardener => "Садовник",
            Skill::Cook => "Повар",
            Skill::Decorator => "Декоратор",
        }
    }
    pub fn perk(self, lvl: u32) -> String {
        let l = lvl.saturating_sub(1);
        match self {
            Skill::Gardener => format!(
                "Рост твоих посадок +{}%, шанс двойного урожая {}%",
                l * 5,
                10 + l * 3
            ),
            Skill::Cook => format!("Шанс приготовить две порции {}%", 5 + l * 5),
            Skill::Decorator => format!("Скидка в лавке {}%, уют +{}%", l * 3, l * 10),
        }
    }
}

/// A shared goal for the whole island (cooperative mini-quests).
#[derive(Clone, Copy, Debug)]
pub enum GoalKind {
    Harvest(Option<Crop>, u32),
    Water(u32),
    Cook(Option<Dish>, u32),
    DistinctDishes(u32),
    PlaceDecor(u32),
    Cozy(u32),
    Sell(u32),
    Pets(u32),
}

pub struct Goal {
    pub title: &'static str,
    pub kind: GoalKind,
    pub xp: u32,
    pub coins: u32,
}

pub const GOALS: &[Goal] = &[
    Goal { title: "Первый урожай: собрать 3 пшеницы", kind: GoalKind::Harvest(Some(Crop::Wheat), 3), xp: 20, coins: 10 },
    Goal { title: "Полить 8 грядок (можно и чужие!)", kind: GoalKind::Water(8), xp: 15, coins: 10 },
    Goal { title: "Испечь хлеб на общей кухне", kind: GoalKind::Cook(Some(Dish::Bread), 1), xp: 25, coins: 15 },
    Goal { title: "Поставить 2 украшения", kind: GoalKind::PlaceDecor(2), xp: 20, coins: 10 },
    Goal { title: "Собрать 8 моркови всей компанией", kind: GoalKind::Harvest(Some(Crop::Carrot), 8), xp: 30, coins: 20 },
    Goal { title: "Продать урожай на 60 монет", kind: GoalKind::Sell(60), xp: 20, coins: 0 },
    Goal { title: "Сделать остров уютным (уют 8)", kind: GoalKind::Cozy(8), xp: 35, coins: 20 },
    Goal { title: "Собрать 6 капусты", kind: GoalKind::Harvest(Some(Crop::Cabbage), 6), xp: 35, coins: 20 },
    Goal { title: "Приготовить салат", kind: GoalKind::Cook(Some(Dish::Salad), 1), xp: 30, coins: 20 },
    Goal { title: "Приютить питомца", kind: GoalKind::Pets(1), xp: 40, coins: 25 },
    Goal { title: "Собрать 40 любых овощей", kind: GoalKind::Harvest(None, 40), xp: 50, coins: 30 },
    Goal { title: "Приготовить 3 разных блюда", kind: GoalKind::DistinctDishes(3), xp: 50, coins: 30 },
    Goal { title: "Собрать 10 картофеля", kind: GoalKind::Harvest(Some(Crop::Potato), 10), xp: 50, coins: 30 },
    Goal { title: "Уют острова 20", kind: GoalKind::Cozy(20), xp: 60, coins: 40 },
    Goal { title: "Приготовить 10 блюд", kind: GoalKind::Cook(None, 10), xp: 70, coins: 50 },
    Goal { title: "Полить 60 грядок", kind: GoalKind::Water(60), xp: 60, coins: 40 },
    Goal { title: "Два питомца на острове", kind: GoalKind::Pets(2), xp: 80, coins: 50 },
    Goal { title: "Собрать 150 овощей", kind: GoalKind::Harvest(None, 150), xp: 120, coins: 80 },
];

pub const DAY_SECS: f32 = 300.0;
pub const DAYS_PER_SEASON: u32 = 3;
pub const CAN_CAPACITY: u32 = 15;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_recipe_uses_unlocked_crops() {
        for d in Dish::ALL {
            for (c, n) in d.recipe() {
                assert!(*n > 0);
                assert!(c.unlock_level() <= d.unlock_level(), "{:?} needs {:?}", d, c);
            }
        }
    }

    #[test]
    fn dishes_are_worth_more_than_ingredients() {
        for d in Dish::ALL {
            let raw: u32 = d.recipe().iter().map(|(c, n)| c.sell_price() * n).sum();
            assert!(d.sell_price() > raw, "{:?}: {} <= {}", d, d.sell_price(), raw);
        }
    }

    #[test]
    fn ownership_rules() {
        assert!(Owner::Shared.can_build(Who::Mila));
        assert!(Owner::You.can_build(Who::You));
        assert!(!Owner::You.can_build(Who::Mila));
        assert!(!Owner::Ksyu.can_build(Who::You));
    }

    #[test]
    fn xp_curve_increases() {
        for l in 1..20 {
            assert!(xp_for_next(l + 1) > xp_for_next(l));
        }
    }
}
