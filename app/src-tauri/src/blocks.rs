//! What the Blocks screen shows, and how picked files become list entries.

use std::path::Path;

use niwa_blocker::catalog::{InstalledApps, CATEGORIES};
use niwa_blocker::identity::file_identity;
use niwa_blocker::installed::{suggested_allowed, Installed};
use niwa_core::{AppEntry, BlockListConfig, ExceptionEntry, ListKind, Mode};

use crate::views::{BlockListView, CategoryView, GamesFound};

pub const DEFAULT_LIST_NAME: &str = "Deep work";

pub fn view(name: &str, config: &BlockListConfig, installed: &Installed) -> BlockListView {
    let apps = InstalledApps::scan();
    BlockListView {
        name: name.to_owned(),
        config: config.clone(),
        found: GamesFound {
            steam: installed.steam.games.len(),
            epic: installed.epic.games.len(),
            riot: installed.riot.games.len(),
        },
        categories: CATEGORIES
            .iter()
            .map(|category| CategoryView {
                id: category.id,
                label: category.label,
                apps: category.apps.iter().map(|app| app.name).collect(),
                found: apps.count(category),
            })
            .collect(),
        suggested: suggested_allowed(&apps),
        summary: summary(config),
    }
}

/// "Games, Chat +2 apps", led by "Strict" when the list is strict.
fn summary(config: &BlockListConfig) -> String {
    let strictly = |text: String| match config.mode {
        Mode::Strict => format!("Strict · {text}"),
        Mode::Gentle => text,
    };
    if config.kind == ListKind::AllowOnly {
        let allowed = match config.allowed.len() {
            0 => "Only Windows and Niwa allowed".to_owned(),
            1 => "Only 1 app allowed".to_owned(),
            n => format!("Only {n} apps allowed"),
        };
        return strictly(allowed);
    }
    let categories = config
        .categories
        .iter()
        .filter_map(|id| CATEGORIES.iter().find(|category| category.id == id))
        .map(|category| match category.id {
            "chat" => "Chat",
            "media" => "Media",
            "browsers" => "Browsers",
            _ => category.label,
        });
    // Games are one category on the Blocks screen, so the summary says so once.
    let games = (config.steam || config.epic || config.riot).then_some("Games");
    let launchers: Vec<&str> = games
        .into_iter()
        .chain(categories)
        .collect();
    let apps = match config.apps.len() {
        0 => None,
        1 => Some("1 app".to_owned()),
        n => Some(format!("{n} apps")),
    };
    let blocked = match (launchers.is_empty(), apps) {
        (true, None) => "Nothing blocked yet".to_owned(),
        (true, Some(apps)) => apps,
        (false, None) => launchers.join(", "),
        (false, Some(apps)) => format!("{} +{apps}", launchers.join(", ")),
    };
    strictly(blocked)
}

/// Describes an executable the user picked to block.
pub fn app_entry(path: &Path) -> AppEntry {
    let identity = file_identity(path);
    let non_empty = |text: &String| !text.is_empty();
    let product_name = identity
        .as_ref()
        .map(|identity| identity.product_name.clone())
        .filter(non_empty);
    AppEntry {
        name: product_name.clone().unwrap_or_else(|| file_stem(path)),
        path: path.to_string_lossy().into_owned(),
        original_filename: identity
            .map(|identity| identity.original_filename)
            .filter(non_empty),
        product_name,
        known: None,
    }
}

/// Describes a program or folder the user picked to keep allowed.
pub fn exception_entry(path: &Path, installed: &Installed) -> ExceptionEntry {
    let name = installed
        .name_for(path)
        .or_else(|| path.is_file().then(|| app_entry(path).name))
        .unwrap_or_else(|| file_stem(path));
    ExceptionEntry {
        name,
        path: path.to_string_lossy().into_owned(),
    }
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(steam: bool, epic: bool, riot: bool, apps: usize) -> BlockListConfig {
        BlockListConfig {
            steam,
            epic,
            riot,
            apps: (0..apps)
                .map(|i| AppEntry {
                    name: format!("App {i}"),
                    path: format!(r"C:\Apps\app{i}.exe"),
                    product_name: None,
                    original_filename: None,
                    known: None,
                })
                .collect(),
            ..BlockListConfig::default()
        }
    }

    #[test]
    fn summary_lists_games_then_apps() {
        assert_eq!(summary(&config(true, true, true, 2)), "Games +2 apps");
        assert_eq!(summary(&config(true, false, true, 0)), "Games");
        assert_eq!(summary(&config(false, false, false, 1)), "1 app");
        assert_eq!(summary(&config(false, false, false, 0)), "Nothing blocked yet");
    }

    #[test]
    fn summary_says_when_a_list_is_strict() {
        let strict = BlockListConfig {
            mode: Mode::Strict,
            ..config(true, false, false, 0)
        };
        assert_eq!(summary(&strict), "Strict · Games");
    }

    #[test]
    fn summary_names_categories_after_games() {
        let mut list = config(true, false, false, 1);
        list.categories = vec!["chat".into(), "browsers".into()];
        assert_eq!(summary(&list), "Games, Chat, Browsers +1 app");
    }

    #[test]
    fn summary_of_an_allow_only_list_counts_what_is_allowed() {
        let mut list = config(true, true, true, 3);
        list.kind = ListKind::AllowOnly;
        assert_eq!(summary(&list), "Only Windows and Niwa allowed");
        list.allowed = config(false, false, false, 4).apps;
        list.mode = Mode::Strict;
        assert_eq!(summary(&list), "Strict · Only 4 apps allowed");
    }

    #[test]
    fn folders_that_are_not_games_take_their_own_name() {
        let installed = Installed {
            steam_root: None,
            steam: Default::default(),
            epic: Default::default(),
            riot: Default::default(),
        };
        let entry = exception_entry(Path::new(r"C:\Tools\Thing"), &installed);
        assert_eq!(entry.name, "Thing");
    }
}
