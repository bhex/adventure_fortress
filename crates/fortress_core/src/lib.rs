pub mod adventurers;
pub mod content;
pub mod engine;
pub mod events;
pub mod fortress;
pub mod game_state;
pub mod inhabitants;
pub mod player;
pub mod region;
pub mod resources;
pub mod rng;
pub mod skills;

pub use adventurers::{generate_adventurer, Adventurer, AdventurerClass};
pub use engine::{choice_availability, eligible_events, resolve, roll, ChoiceAvailability};
pub use events::{Choice, Effect, Event, EventResult, StatCheck};
pub use fortress::{Fortress, Upgrade};
pub use game_state::{
    BuildAvailability, GameState, ADVENTURER_MIN_REPUTATION, LEVEL_UP_INTERVAL, MAX_ADVENTURERS,
    SAVE_VERSION,
};
pub use inhabitants::{generate_inhabitant, Inhabitant, InhabitantManager, Role, Trait};
pub use player::{ability_offers, ClassKind, PlayerAbility, PlayerCharacter, StatKind, Stats};
pub use region::{darkness_band, DarknessBand, Region, Site, SiteKind};
pub use resources::{amount_phrase, band_for, ResourceDelta, ResourceKind, Resources, StockBand};
pub use rng::GameRng;
pub use skills::{tier_for_xp, Skill, SkillSet, SkillTier};
