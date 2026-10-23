//! What a block list contains, as stored and as shown on the Blocks screen.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Gentle,
    Strict,
}

impl Mode {
    pub fn id(self) -> &'static str {
        match self {
            Mode::Gentle => "gentle",
            Mode::Strict => "strict",
        }
    }

    pub fn from_id(id: &str) -> Option<Mode> {
        match id {
            "gentle" => Some(Mode::Gentle),
            "strict" => Some(Mode::Strict),
            _ => None,
        }
    }

    /// How long a request to end a session early waits before the plant
    /// withers (changed Oct 5, 2026): five minutes in Gentle; in Strict a
    /// third of the session, in whole minutes rounded up, and never less than
    /// Gentle's. The card's `exitWaitMinutes` in `lib/format.ts` mirrors this.
    pub fn exit_wait_secs(self, planned_minutes: u32) -> u64 {
        const GENTLE_MINUTES: u32 = 5;
        let minutes = match self {
            Mode::Gentle => GENTLE_MINUTES,
            Mode::Strict => planned_minutes.div_ceil(3).max(GENTLE_MINUTES),
        };
        u64::from(minutes) * 60
    }
}

/// What a list does: close what it names, or close everything it doesn't.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListKind {
    /// Close the games, categories and apps the list names.
    #[default]
    Block,
    /// Close every app with a taskbar window that isn't allowed. Windows
    /// itself, Niwa and anti-cheat are always left alone.
    AllowOnly,
}

/// An app the user picked to block or to allow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub name: String,
    pub path: String,
    /// From the executable's version resource, so the app is still recognised
    /// after an update moves it.
    #[serde(default)]
    pub product_name: Option<String>,
    #[serde(default)]
    pub original_filename: Option<String>,
    /// Set for an app Niwa knows by name (`niwa_blocker::catalog`), such as
    /// the work apps suggested for an allow list. It is then recognised the
    /// way the catalog describes, wherever it is installed, and `path` is
    /// only a hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known: Option<String>,
}

/// An executable or folder that stays allowed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BlockListConfig {
    pub mode: Mode,
    pub kind: ListKind,
    pub steam: bool,
    pub epic: bool,
    pub riot: bool,
    /// Ready-made groups of apps to block, by id, such as "chat".
    pub categories: Vec<String>,
    pub apps: Vec<AppEntry>,
    pub exceptions: Vec<ExceptionEntry>,
    /// For an allow-only list: the apps that may stay open. Kept apart from
    /// `apps` so switching kinds loses neither.
    pub allowed: Vec<AppEntry>,
}

impl BlockListConfig {
    /// True when the list would block something.
    pub fn blocks_anything(&self) -> bool {
        self.kind == ListKind::AllowOnly
            || self.steam
            || self.epic
            || self.riot
            || !self.categories.is_empty()
            || !self.apps.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ending_early_waits_five_minutes_in_gentle_and_a_third_in_strict() {
        assert_eq!(Mode::Gentle.exit_wait_secs(25), 5 * 60);
        assert_eq!(Mode::Gentle.exit_wait_secs(300), 5 * 60);
        assert_eq!(Mode::Strict.exit_wait_secs(60), 20 * 60);
        assert_eq!(Mode::Strict.exit_wait_secs(90), 30 * 60);
        // Rounded up to whole minutes.
        assert_eq!(Mode::Strict.exit_wait_secs(25), 9 * 60);
        assert_eq!(Mode::Strict.exit_wait_secs(50), 17 * 60);
        // Never easier than Gentle.
        assert_eq!(Mode::Strict.exit_wait_secs(10), 5 * 60);
        assert_eq!(Mode::Strict.exit_wait_secs(300), 100 * 60);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let config: BlockListConfig = serde_json::from_str(r#"{ "steam": true }"#).unwrap();
        assert!(config.steam);
        assert!(!config.epic);
        assert_eq!(config.mode, Mode::Gentle);
        assert_eq!(config.kind, ListKind::Block);
        assert!(config.apps.is_empty());
        assert!(config.categories.is_empty());
        assert!(config.allowed.is_empty());
    }

    #[test]
    fn lists_saved_with_websites_still_load() {
        // Website blocking was removed on Oct 6, 2026; lists saved before
        // then still have these fields.
        let config: BlockListConfig =
            serde_json::from_str(r#"{ "riot": true, "siteCategories": ["social"], "sites": ["example.com"] }"#).unwrap();
        assert!(config.riot);
        assert!(config.blocks_anything());
        let only_sites: BlockListConfig = serde_json::from_str(r#"{ "siteCategories": ["video"] }"#).unwrap();
        assert!(!only_sites.blocks_anything());
    }

    #[test]
    fn allow_only_always_blocks_something() {
        let config = BlockListConfig {
            kind: ListKind::AllowOnly,
            ..Default::default()
        };
        assert!(config.blocks_anything());
    }

    #[test]
    fn kinds_have_stable_names() {
        assert_eq!(serde_json::to_string(&ListKind::AllowOnly).unwrap(), r#""allowOnly""#);
        assert_eq!(serde_json::to_string(&ListKind::Block).unwrap(), r#""block""#);
    }

    #[test]
    fn round_trips_through_json() {
        let config = BlockListConfig {
            mode: Mode::Gentle,
            steam: true,
            epic: false,
            riot: true,
            categories: vec!["chat".into()],
            apps: vec![AppEntry {
                name: "Discord".into(),
                path: r"C:\Apps\Discord.exe".into(),
                product_name: Some("Discord".into()),
                original_filename: None,
                known: None,
            }],
            exceptions: vec![ExceptionEntry {
                name: "Wallpaper Engine".into(),
                path: r"C:\Steam\steamapps\common\wallpaper_engine".into(),
            }],
            kind: ListKind::AllowOnly,
            allowed: vec![AppEntry {
                name: "Google Chrome".into(),
                path: String::new(),
                product_name: None,
                original_filename: None,
                known: Some("chrome".into()),
            }],
        };
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(serde_json::from_str::<BlockListConfig>(&json).unwrap(), config);
    }

    #[test]
    fn an_empty_list_blocks_nothing() {
        assert!(!BlockListConfig::default().blocks_anything());
        assert!(BlockListConfig {
            epic: true,
            ..Default::default()
        }
        .blocks_anything());
    }
}
