//! The plants, what each one needs, and how a plant grows over a session.
//!
//! The thresholds are the spec's starting values, to be tuned in playtesting.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The shortest and longest session a custom duration allows, in minutes.
pub const MIN_SESSION_MINUTES: u32 = 10;
pub const MAX_SESSION_MINUTES: u32 = 300;

/// Seed, sprout, sapling, young, fully grown.
pub const STAGES: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Species {
    Sprout,
    Succulent,
    Camellia,
    Pine,
    Fern,
    Bamboo,
    Hydrangea,
    Maple,
    Sakura,
    Wisteria,
    Ginkgo,
}

impl Species {
    /// In unlock order, which is also the order the seed picker shows them.
    pub const ALL: [Species; 11] = [
        Species::Sprout,
        Species::Succulent,
        Species::Camellia,
        Species::Pine,
        Species::Fern,
        Species::Bamboo,
        Species::Hydrangea,
        Species::Maple,
        Species::Sakura,
        Species::Wisteria,
        Species::Ginkgo,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Species::Sprout => "sprout",
            Species::Succulent => "succulent",
            Species::Camellia => "camellia",
            Species::Pine => "pine",
            Species::Fern => "fern",
            Species::Bamboo => "bamboo",
            Species::Hydrangea => "hydrangea",
            Species::Maple => "maple",
            Species::Sakura => "sakura",
            Species::Wisteria => "wisteria",
            Species::Ginkgo => "ginkgo",
        }
    }

    pub fn from_id(id: &str) -> Option<Species> {
        Species::ALL.into_iter().find(|species| species.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Species::Sprout => "Sprout",
            Species::Succulent => "Succulent",
            Species::Camellia => "Camellia",
            Species::Pine => "Pine",
            Species::Fern => "Fern",
            Species::Bamboo => "Bamboo",
            Species::Hydrangea => "Hydrangea",
            Species::Maple => "Maple",
            Species::Sakura => "Sakura",
            Species::Wisteria => "Wisteria",
            Species::Ginkgo => "Ginkgo",
        }
    }

    /// The shortest session this seed can be planted for. Zero means any.
    pub fn min_minutes(self) -> u32 {
        match self {
            Species::Sprout => 0,
            Species::Fern => 25,
            Species::Succulent => 30,
            Species::Camellia | Species::Hydrangea => 45,
            Species::Pine => 50,
            Species::Bamboo | Species::Sakura => 60,
            Species::Maple | Species::Wisteria => 90,
            Species::Ginkgo => 120,
        }
    }

    /// Total focused hours needed before this seed is available.
    pub fn unlock_hours(self) -> u32 {
        match self {
            Species::Sprout | Species::Succulent | Species::Camellia | Species::Pine => 0,
            Species::Fern => 2,
            Species::Bamboo => 5,
            Species::Hydrangea => 8,
            Species::Maple => 10,
            Species::Sakura => 15,
            Species::Wisteria => 25,
            Species::Ginkgo => 40,
        }
    }

    /// `focused_minutes` is the total from finished sessions; withered
    /// sessions don't count.
    pub fn is_unlocked(self, focused_minutes: u32) -> bool {
        focused_minutes >= self.unlock_hours() * 60
    }

    pub fn fits_session(self, planned_minutes: u32) -> bool {
        planned_minutes >= self.min_minutes()
    }
}

/// The next species the user hasn't unlocked yet, if any.
pub fn next_unlock(focused_minutes: u32) -> Option<Species> {
    Species::ALL
        .into_iter()
        .filter(|species| !species.is_unlocked(focused_minutes))
        .min_by_key(|species| species.unlock_hours())
}

/// The growth stage, 1 to 5, for a session that is `elapsed` into `total`.
///
/// Stages begin at 0, 25, 50 and 75% of the session; stage 5 is only reached
/// when the session finishes.
pub fn stage(elapsed: Duration, total: Duration) -> u8 {
    if total.is_zero() || elapsed >= total {
        return STAGES;
    }
    let quarter = elapsed.as_millis() * 4 / total.as_millis();
    quarter as u8 + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minutes(m: u64) -> Duration {
        Duration::from_secs(m * 60)
    }

    #[test]
    fn stages_begin_at_each_quarter() {
        let total = minutes(100);
        for (elapsed, expected) in [
            (0, 1),
            (24, 1),
            (25, 2),
            (49, 2),
            (50, 3),
            (63, 3),
            (74, 3),
            (75, 4),
            (99, 4),
            (100, 5),
            (120, 5),
        ] {
            assert_eq!(stage(minutes(elapsed), total), expected, "at {elapsed} min");
        }
    }

    #[test]
    fn last_second_is_still_stage_four() {
        let total = minutes(25);
        assert_eq!(stage(total - Duration::from_millis(1), total), 4);
    }

    #[test]
    fn starting_species_need_no_hours() {
        for species in [
            Species::Sprout,
            Species::Succulent,
            Species::Camellia,
            Species::Pine,
        ] {
            assert!(species.is_unlocked(0), "{species:?}");
        }
        for species in [
            Species::Fern,
            Species::Bamboo,
            Species::Hydrangea,
            Species::Maple,
            Species::Sakura,
            Species::Wisteria,
            Species::Ginkgo,
        ] {
            assert!(!species.is_unlocked(0), "{species:?}");
        }
    }

    #[test]
    fn species_unlock_at_their_hour_totals() {
        assert!(!Species::Bamboo.is_unlocked(5 * 60 - 1));
        assert!(Species::Bamboo.is_unlocked(5 * 60));
        assert!(!Species::Maple.is_unlocked(10 * 60 - 1));
        assert!(Species::Maple.is_unlocked(10 * 60));
        assert!(!Species::Wisteria.is_unlocked(25 * 60 - 1));
        assert!(Species::Wisteria.is_unlocked(25 * 60));
        assert!(!Species::Fern.is_unlocked(2 * 60 - 1));
        assert!(Species::Fern.is_unlocked(2 * 60));
        assert!(!Species::Hydrangea.is_unlocked(8 * 60 - 1));
        assert!(Species::Hydrangea.is_unlocked(8 * 60));
        assert!(!Species::Sakura.is_unlocked(15 * 60 - 1));
        assert!(Species::Sakura.is_unlocked(15 * 60));
        assert!(!Species::Ginkgo.is_unlocked(40 * 60 - 1));
        assert!(Species::Ginkgo.is_unlocked(40 * 60));
    }

    #[test]
    fn seed_picker_order_follows_the_unlocks() {
        // The picker lists seeds in this order and picks the last one a
        // session can grow, so it must climb by hours, then by session length.
        let keys: Vec<(u32, u32)> = Species::ALL
            .iter()
            .map(|species| (species.unlock_hours(), species.min_minutes()))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn seeds_need_a_long_enough_session() {
        assert!(Species::Sprout.fits_session(MIN_SESSION_MINUTES));
        assert!(!Species::Succulent.fits_session(29));
        assert!(Species::Succulent.fits_session(30));
        assert!(!Species::Camellia.fits_session(44));
        assert!(Species::Camellia.fits_session(45));
        assert!(!Species::Pine.fits_session(49));
        assert!(Species::Pine.fits_session(50));
        assert!(!Species::Bamboo.fits_session(59));
        assert!(Species::Bamboo.fits_session(60));
        assert!(!Species::Maple.fits_session(89));
        assert!(Species::Wisteria.fits_session(90));
        assert!(!Species::Ginkgo.fits_session(119));
        assert!(Species::Ginkgo.fits_session(120));
    }

    #[test]
    fn next_unlock_walks_up_the_thresholds() {
        assert_eq!(next_unlock(0), Some(Species::Fern));
        assert_eq!(next_unlock(2 * 60), Some(Species::Bamboo));
        assert_eq!(next_unlock(5 * 60), Some(Species::Hydrangea));
        assert_eq!(next_unlock(8 * 60), Some(Species::Maple));
        assert_eq!(next_unlock(10 * 60), Some(Species::Sakura));
        assert_eq!(next_unlock(15 * 60), Some(Species::Wisteria));
        assert_eq!(next_unlock(25 * 60), Some(Species::Ginkgo));
        assert_eq!(next_unlock(40 * 60), None);
    }

    #[test]
    fn ids_round_trip() {
        for species in Species::ALL {
            assert_eq!(Species::from_id(species.id()), Some(species));
        }
        assert_eq!(Species::from_id("oak"), None);
        // Ids are stored in the database, so they must match serde's names too.
        for species in Species::ALL {
            assert_eq!(serde_json::to_string(&species).unwrap(), format!("\"{}\"", species.id()));
        }
    }
}
