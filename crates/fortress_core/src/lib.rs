pub mod adventurers;
pub mod battle;
pub mod content;
pub mod engine;
pub mod events;
pub mod fortress;
pub mod game_state;
pub mod inhabitants;
pub mod items;
pub mod player;
pub mod region;
pub mod resources;
pub mod rng;
pub mod skills;
pub mod world;

pub use adventurers::{generate_adventurer, Adventurer, AdventurerClass};
pub use battle::{fight_battle, BattleReport};
pub use engine::{
    auto_pick, choice_availability, describe_effects, eligible_events, resolve, roll,
    stat_check_odds, ChoiceAvailability,
};
pub use events::{Choice, Effect, Event, EventResult, StatCheck};
pub use fortress::{
    level_numeral, BuildOutcome, BuildProject, Building, Fortress, FortressFeature, SettlementTier,
    Upgrade, HOUSING_PLOTS, MAX_BUILDING_LEVEL,
};
pub use game_state::{
    BuildAvailability, GameState, ADVENTURER_MIN_REPUTATION, MAX_ADVENTURERS, SAVE_VERSION,
};
pub use inhabitants::{generate_inhabitant, Inhabitant, InhabitantManager, Role, Trait};
pub use items::{Enchant, Item, ItemForm, ItemKind, ItemStock, Loadout, Material, Quality};
pub use player::{ClassKind, PlayerCharacter, StatKind, Stats};
pub use region::{darkness_band, DarknessBand, Region, Site, SiteKind};
pub use resources::{amount_phrase, band_for, ResourceDelta, ResourceKind, Resources, StockBand};
pub use rng::GameRng;
pub use skills::{tier_for_xp, Skill, SkillSet, SkillTier};
pub use world::{Season, Weather, World, DAYS_PER_SEASON};
