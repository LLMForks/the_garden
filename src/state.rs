//! Shared island state: storage, coins, island level, personal skills, goals.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::data::*;

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct Stats {
    pub harvested: BTreeMap<Crop, u32>,
    pub harvested_total: u32,
    pub watered: u32,
    pub cooked: BTreeMap<Dish, u32>,
    pub cooked_total: u32,
    pub placed: u32,
    pub sold: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum PetKind {
    Cat,
    Fox,
    Cat2,
}

impl PetKind {
    pub fn name(self) -> &'static str {
        match self {
            PetKind::Cat => "кошка Булочка",
            PetKind::Fox => "лисёнок Рыжик",
            PetKind::Cat2 => "кот Пломбир",
        }
    }
    pub fn cozy_needed(self) -> u32 {
        match self {
            PetKind::Cat => 6,
            PetKind::Fox => 14,
            PetKind::Cat2 => 24,
        }
    }
}

#[derive(Resource, Clone, Serialize, Deserialize, Debug)]
pub struct Game {
    pub storage: BTreeMap<Item, u32>,
    pub coins: u32,
    pub level: u32,
    pub xp: u32,
    pub skills: BTreeMap<String, (u32, u32)>,
    pub stats: Stats,
    pub goal_idx: usize,
    pub goal_start: Stats,
    pub can_water: u32,
    pub pets: Vec<PetKind>,
    pub tutorial: u32,
    pub cozy: u32,
}

impl Default for Game {
    fn default() -> Self {
        let mut storage = BTreeMap::new();
        storage.insert(Item::Seeds(Crop::Wheat), 8);
        storage.insert(Item::Seeds(Crop::Carrot), 5);
        storage.insert(Item::Decor(Decor::Fence), 4);
        storage.insert(Item::Decor(Decor::Flowerbed), 1);
        let mut skills = BTreeMap::new();
        for s in Skill::ALL {
            skills.insert(s.name().to_string(), (1, 0));
        }
        Game {
            storage,
            coins: 20,
            level: 1,
            xp: 0,
            skills,
            stats: Stats::default(),
            goal_idx: 0,
            goal_start: Stats::default(),
            can_water: CAN_CAPACITY,
            pets: vec![],
            tutorial: 0,
            cozy: 0,
        }
    }
}

/// Things that happened, for toasts, the activity feed and sounds.
#[derive(Event, Clone, Debug)]
pub enum Note {
    /// Small message in the activity feed (bottom-left).
    Feed(String),
    /// Prominent toast with an icon.
    Toast(String, usize),
    LevelUp(u32),
    SkillUp(Skill, u32),
    GoalDone(String),
}

impl Game {
    pub fn count(&self, it: Item) -> u32 {
        *self.storage.get(&it).unwrap_or(&0)
    }
    pub fn add(&mut self, it: Item, n: u32) {
        *self.storage.entry(it).or_insert(0) += n;
    }
    pub fn take(&mut self, it: Item, n: u32) -> bool {
        let c = self.count(it);
        if c < n {
            return false;
        }
        if c == n {
            self.storage.remove(&it);
        } else {
            self.storage.insert(it, c - n);
        }
        true
    }
    pub fn skill(&self, s: Skill) -> u32 {
        self.skills.get(s.name()).map(|v| v.0).unwrap_or(1)
    }
    pub fn skill_progress(&self, s: Skill) -> (u32, u32) {
        let (l, x) = self.skills.get(s.name()).copied().unwrap_or((1, 0));
        (x, skill_xp_for_next(l))
    }
    pub fn add_xp(&mut self, xp: u32, notes: &mut EventWriter<Note>) {
        self.xp += xp;
        while self.xp >= xp_for_next(self.level) {
            self.xp -= xp_for_next(self.level);
            self.level += 1;
            notes.write(Note::LevelUp(self.level));
        }
    }
    pub fn add_skill_xp(&mut self, s: Skill, xp: u32, notes: &mut EventWriter<Note>) {
        let e = self.skills.entry(s.name().to_string()).or_insert((1, 0));
        e.1 += xp;
        while e.1 >= skill_xp_for_next(e.0) {
            e.1 -= skill_xp_for_next(e.0);
            e.0 += 1;
            notes.write(Note::SkillUp(s, e.0));
        }
    }
    pub fn discount(&self) -> f32 {
        1.0 - 0.03 * (self.skill(Skill::Decorator) - 1).min(10) as f32
    }
    pub fn price_of(&self, base: u32) -> u32 {
        ((base as f32 * self.discount()).round() as u32).max(1)
    }
    pub fn crop_unlocked(&self, c: Crop) -> bool {
        self.level >= c.unlock_level()
    }
    pub fn dish_unlocked(&self, d: Dish) -> bool {
        self.level >= d.unlock_level()
    }
    pub fn decor_available(&self, d: Decor, season: Season) -> bool {
        self.level >= d.unlock_level() && d.season().is_none_or(|s| s == season)
    }

    pub fn goal_progress(&self) -> Option<(&'static Goal, u32, u32)> {
        let g = GOALS.get(self.goal_idx)?;
        let s = &self.stats;
        let b = &self.goal_start;
        let (have, need) = match g.kind {
            GoalKind::Harvest(Some(c), n) => (
                s.harvested.get(&c).unwrap_or(&0) - b.harvested.get(&c).unwrap_or(&0),
                n,
            ),
            GoalKind::Harvest(None, n) => (s.harvested_total - b.harvested_total, n),
            GoalKind::Water(n) => (s.watered - b.watered, n),
            GoalKind::Cook(Some(d), n) => {
                (s.cooked.get(&d).unwrap_or(&0) - b.cooked.get(&d).unwrap_or(&0), n)
            }
            GoalKind::Cook(None, n) => (s.cooked_total - b.cooked_total, n),
            GoalKind::DistinctDishes(n) => (s.cooked.len() as u32, n),
            GoalKind::PlaceDecor(n) => (s.placed - b.placed, n),
            GoalKind::Cozy(n) => (self.cozy, n),
            GoalKind::Sell(n) => (s.sold - b.sold, n),
            GoalKind::Pets(n) => (self.pets.len() as u32, n),
        };
        Some((g, have.min(need), need))
    }

    /// Completes the current goal if it is done. Returns true if something happened.
    pub fn check_goal(&mut self, notes: &mut EventWriter<Note>) -> bool {
        if let Some((g, have, need)) = self.goal_progress() {
            if have >= need {
                let (xp, coins, title) = (g.xp, g.coins, g.title);
                self.goal_idx += 1;
                self.goal_start = self.stats.clone();
                self.coins += coins;
                notes.write(Note::GoalDone(title.to_string()));
                self.add_xp(xp, notes);
                return true;
            }
        }
        false
    }
}

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<Note>()
            .init_resource::<Game>()
            .add_systems(Update, goals_system);
    }
}

fn goals_system(mut game: ResMut<Game>, mut notes: EventWriter<Note>) {
    if game.is_changed() {
        game.check_goal(&mut notes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_add_take() {
        let mut g = Game::default();
        let it = Item::Crop(Crop::Potato);
        assert_eq!(g.count(it), 0);
        g.add(it, 3);
        assert!(!g.take(it, 4));
        assert!(g.take(it, 3));
        assert_eq!(g.count(it), 0);
        assert!(!g.storage.contains_key(&it));
    }

    #[test]
    fn starting_seeds_are_unlocked() {
        let g = Game::default();
        for (it, _) in &g.storage {
            if let Item::Seeds(c) = it {
                assert!(g.crop_unlocked(*c));
            }
        }
    }

    #[test]
    fn seasonal_decor_only_in_season() {
        let mut g = Game::default();
        g.level = 10;
        assert!(g.decor_available(Decor::Snowman, Season::Winter));
        assert!(!g.decor_available(Decor::Snowman, Season::Summer));
        assert!(g.decor_available(Decor::Fence, Season::Summer));
    }
}
