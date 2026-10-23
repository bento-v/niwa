//! The shapes sent to the card UI. Field names are camelCase on the wire.

use niwa_core::store::{GardenPlant, Outcome};
use niwa_core::{AppEntry, BlockListConfig, Species};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeciesView {
    pub id: &'static str,
    pub name: &'static str,
    pub min_minutes: u32,
    pub unlock_hours: u32,
    pub unlocked: bool,
}

impl SpeciesView {
    pub fn new(species: Species, focused_minutes: u32) -> Self {
        SpeciesView {
            id: species.id(),
            name: species.name(),
            min_minutes: species.min_minutes(),
            unlock_hours: species.unlock_hours(),
            unlocked: species.is_unlocked(focused_minutes),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NextUnlock {
    pub name: &'static str,
    pub hours: u32,
}

/// An app that was open when the session started and is about to be closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosingView {
    pub label: String,
    pub secs_left: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub species: &'static str,
    pub species_name: &'static str,
    pub planned_minutes: u32,
    pub mode: &'static str,
    pub list_name: String,
    pub remaining_secs: u32,
    pub total_secs: u32,
    pub stage: u8,
    pub slips: u32,
    /// Short labels for what's being blocked, such as "All Steam games".
    pub blocking: Vec<String>,
    pub closing: Vec<ClosingView>,
    /// Strict only: seconds until a requested early end takes effect.
    pub exit_secs_left: Option<u32>,
    /// Started by a schedule.
    pub scheduled: bool,
}

/// Niwa just closed an app, slip or not: the card and the mini timer show it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosedView {
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlipView {
    pub label: String,
    pub slips: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndedView {
    pub outcome: &'static str,
    pub species: &'static str,
    pub species_name: &'static str,
    pub planned_minutes: u32,
    pub remaining_secs: u32,
    pub slips: u32,
    pub today_minutes: u32,
    pub streak_days: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamesFound {
    pub steam: usize,
    pub epic: usize,
    pub riot: usize,
}

/// A ready-made category of apps on the Blocks screen.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryView {
    pub id: &'static str,
    pub label: &'static str,
    /// The apps it covers, for the row's second line: "Discord, Telegram, WhatsApp…".
    pub apps: Vec<&'static str>,
    /// How many of them are installed here.
    pub found: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockListView {
    pub name: String,
    pub config: BlockListConfig,
    pub found: GamesFound,
    pub categories: Vec<CategoryView>,
    /// The work apps installed here, offered when an allow-only list is empty.
    pub suggested: Vec<AppEntry>,
    /// One line for the start screen, such as "Steam, Epic, Riot +2 apps".
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub session: Option<SessionView>,
    pub focused_minutes: u32,
    pub streak_days: u32,
    pub species: Vec<SpeciesView>,
    pub list: BlockListView,
    /// Whether the Strict service is installed and answering.
    pub strict_available: bool,
    pub min_minutes: u32,
    pub max_minutes: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlantView {
    pub id: i64,
    pub species: &'static str,
    pub species_name: &'static str,
    pub withered: bool,
    /// The session length for a grown plant; how long it lasted for a
    /// withered one.
    pub minutes: u32,
    pub planned_minutes: u32,
    pub mode: &'static str,
    pub slips: u32,
    /// Unix milliseconds, for the UI to format in the user's locale.
    pub created_ms: i64,
}

impl From<GardenPlant> for PlantView {
    fn from(plant: GardenPlant) -> Self {
        let withered = plant.outcome == Outcome::Withered;
        PlantView {
            id: plant.id,
            species: plant.species.id(),
            species_name: plant.species.name(),
            withered,
            minutes: match withered {
                true => plant.focused_seconds / 60,
                false => plant.planned_minutes,
            },
            planned_minutes: plant.planned_minutes,
            mode: plant.mode.id(),
            slips: plant.slip_count,
            created_ms: plant.created * 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GardenView {
    pub plants: Vec<PlantView>,
    pub grown: usize,
    pub withered: usize,
    pub next_unlock: Option<NextUnlock>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemptingView {
    pub name: String,
    pub count: u32,
}

/// This week on the Stats screen.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsView {
    /// Minutes focused each day, Monday first.
    pub days: Vec<u32>,
    /// Which of `days` is today.
    pub today: usize,
    pub week_minutes: u32,
    pub change_percent: Option<i32>,
    pub streak_days: u32,
    pub month_minutes: u32,
    pub completion_percent: Option<u32>,
    /// Every minute spent blocking, across all sessions.
    pub total_minutes: u32,
    pub tempting: Vec<TemptingView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub sound: bool,
    pub start_with_windows: bool,
    pub check_updates: bool,
}

/// A newer version of Niwa, ready to install.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    pub version: String,
    /// Windows will ask for admin: there's no Strict service to install it.
    pub asks_admin: bool,
}
