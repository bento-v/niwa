//! What is installed on this PC, and how a stored block list becomes the
//! list the blocker works from.

use std::path::{Path, PathBuf};

use niwa_core::{AppEntry, BlockListConfig, ExceptionEntry, ListKind};

use crate::blocklist::{AllowOnly, AppRule, BlockList, FileIdentity, GameLibrary, Launcher};
use crate::catalog::{self, InstalledApps};
use crate::{epic, riot, steam};

/// The games each launcher has installed right now.
pub struct Installed {
    pub steam_root: Option<PathBuf>,
    pub steam: GameLibrary,
    pub epic: GameLibrary,
    pub riot: GameLibrary,
}

impl Installed {
    pub fn scan() -> Installed {
        let steam_root = steam::find_steam_root();
        let steam = steam_root
            .as_deref()
            .and_then(|root| steam::scan(root).ok())
            .map(|libraries| libraries.into_library())
            .unwrap_or_default();
        Installed {
            steam_root,
            steam,
            epic: epic::scan(&epic::manifest_dir()),
            riot: riot::scan(&riot::data_dir()),
        }
    }

    /// The list a new user starts with: every game launcher on, and Wallpaper
    /// Engine allowed if it's installed, since it lives in the Steam library
    /// but isn't a game.
    pub fn default_config(&self) -> BlockListConfig {
        let exceptions = self
            .steam
            .games
            .iter()
            .filter(|game| game.name == "Wallpaper Engine")
            .map(|game| ExceptionEntry {
                name: game.name.clone(),
                path: game.install_dir.to_string_lossy().into_owned(),
            })
            .collect();
        BlockListConfig {
            steam: true,
            epic: true,
            riot: true,
            exceptions,
            ..BlockListConfig::default()
        }
    }

    /// The blocker's list for a session, plus short labels for the UI.
    pub fn block_list(self, config: &BlockListConfig) -> (BlockList, Vec<String>) {
        let mut list = BlockList::default();
        let mut labels = Vec::new();
        list.exceptions = config
            .exceptions
            .iter()
            .map(|entry| PathBuf::from(&entry.path))
            .collect();

        if config.kind == ListKind::AllowOnly {
            list.allow_only = Some(AllowOnly {
                allowed: config.allowed.iter().map(app_rule).collect(),
                always: niwa_folder().into_iter().collect(),
                launchers: self.launchers(),
                system_root: system_root(),
            });
            labels.push(match config.allowed.len() {
                1 => "All but 1 app".to_owned(),
                n => format!("All but {n} apps"),
            });
            return (list, labels);
        }

        // Games are one category on the Blocks screen (Oct 5, 2026), so one
        // chip; the list still keeps Steam, Epic and Riot apart.
        if config.steam || config.epic || config.riot {
            labels.push("Games".to_owned());
        }
        // Launchers themselves are never closed, only their games.
        if config.steam {
            list.libraries.push(self.steam);
        }
        if config.epic {
            list.libraries.push(self.epic);
        }
        if config.riot {
            list.libraries.push(self.riot);
        }
        for category in config.categories.iter().filter_map(|id| catalog::category(id)) {
            labels.push(category.label.to_owned());
            list.known.extend(category.apps.iter());
        }
        if !config.apps.is_empty() {
            labels.push(format!("+{}", config.apps.len()));
        }

        list.apps = config.apps.iter().map(app_rule).collect();
        (list, labels)
    }

    /// The launchers installed here, which an allow-only list leaves open.
    fn launchers(&self) -> Vec<Launcher> {
        self.steam_root
            .as_deref()
            .map(steam::launcher)
            .into_iter()
            .chain(epic::launcher())
            .chain(riot::launcher(&riot::data_dir()))
            .collect()
    }

    /// The game installed in a folder, if any: a name for an allowed folder.
    pub fn name_for(&self, path: &Path) -> Option<String> {
        [&self.steam, &self.epic, &self.riot]
            .into_iter()
            .flat_map(|library| &library.games)
            .find(|game| game.install_dir == path)
            .map(|game| game.name.clone())
    }
}

/// The work apps an allow-only list starts with: those installed here.
pub fn suggested_allowed(installed: &InstalledApps) -> Vec<AppEntry> {
    catalog::WORK_APPS
        .iter()
        .filter(|app| installed.has(app))
        .map(|app| AppEntry {
            name: app.name.to_owned(),
            path: String::new(),
            product_name: None,
            original_filename: None,
            known: Some(app.id.to_owned()),
        })
        .collect()
}

/// The folder Niwa runs from. The card and the service live side by side,
/// so either one finds the other's folder this way.
fn niwa_folder() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
}

fn system_root() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
}

fn app_rule(entry: &AppEntry) -> AppRule {
    if let Some(known) = entry.known.as_deref().and_then(catalog::known) {
        return AppRule::Known(known);
    }
    let identity = match (&entry.product_name, &entry.original_filename) {
        (Some(product_name), Some(original_filename)) => Some(FileIdentity {
            product_name: product_name.clone(),
            original_filename: original_filename.clone(),
        }),
        _ => None,
    };
    AppRule::Picked {
        name: entry.name.clone(),
        path: PathBuf::from(&entry.path),
        identity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocklist::Game;

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

    fn installed() -> Installed {
        let library = |source: &str, name: &str, dir: &str| GameLibrary {
            source: source.into(),
            games: vec![Game {
                name: name.into(),
                install_dir: PathBuf::from(dir),
            }],
            ..GameLibrary::default()
        };
        Installed {
            steam_root: Some(PathBuf::from(r"C:\Steam")),
            steam: library("Steam", "Wallpaper Engine", r"C:\Steam\steamapps\common\wallpaper_engine"),
            epic: library("Epic", "Fall Guys", r"C:\Epic\FallGuys"),
            riot: library("Riot", "VALORANT", r"C:\Riot Games\VALORANT\live"),
        }
    }

    #[test]
    fn default_list_allows_wallpaper_engine() {
        let config = installed().default_config();
        assert!(config.steam && config.epic && config.riot);
        assert_eq!(config.exceptions.len(), 1);
        assert_eq!(config.exceptions[0].name, "Wallpaper Engine");
    }

    #[test]
    fn only_enabled_launchers_are_blocked() {
        let (list, labels) = installed().block_list(&config(true, false, true, 3));
        let sources: Vec<_> = list.libraries.iter().map(|l| l.source.as_str()).collect();
        assert_eq!(sources, ["Steam", "Riot"]);
        assert_eq!(labels, ["Games", "+3"]);
        assert_eq!(list.apps.len(), 3);
    }

    #[test]
    fn allow_only_lists_leave_launchers_open_but_not_their_games() {
        let mut config = config(false, false, false, 0);
        config.kind = ListKind::AllowOnly;
        let (list, _) = installed().block_list(&config);
        let blocked = |path: &str| list.check(Path::new(path), || None).is_some();
        assert!(!blocked(r"C:\Steam\steam.exe"));
        assert!(!blocked(r"C:\Steam\bin\cef\cef.win7x64\steamwebhelper.exe"));
        assert!(blocked(r"C:\Steam\steamapps\common\Some Game\game.exe"));
        assert!(blocked(r"C:\Tools\notes.exe"));
    }

    #[test]
    fn game_folders_are_named_after_the_game() {
        assert_eq!(
            installed().name_for(Path::new(r"C:\Epic\FallGuys")),
            Some("Fall Guys".into())
        );
        assert_eq!(installed().name_for(Path::new(r"C:\Tools\Thing")), None);
    }

    #[test]
    fn categories_add_their_apps_and_labels() {
        let mut config = config(false, false, false, 1);
        config.categories = vec!["chat".into(), "media".into(), "no-such-category".into()];
        let (list, labels) = installed().block_list(&config);
        assert_eq!(labels, ["Chat & social", "Music & video", "+1"]);
        assert!(list.known.iter().any(|app| app.id == "discord"));
        assert!(list.known.iter().any(|app| app.id == "spotify"));
        assert!(list.allow_only.is_none());
    }

    #[test]
    fn allow_only_lists_replace_the_block_rules() {
        let mut config = config(true, true, true, 2);
        config.kind = ListKind::AllowOnly;
        config.allowed = suggested_allowed(&InstalledApps::from(&["Google Chrome", "Microsoft Visual Studio Code (User)"], &[]));
        let (list, labels) = installed().block_list(&config);
        let allow_only = list.allow_only.expect("an allow-only list");
        assert_eq!(allow_only.allowed.len(), 2);
        assert!(!allow_only.always.is_empty());
        assert!(list.libraries.is_empty() && list.apps.is_empty() && list.known.is_empty());
        assert_eq!(labels, ["All but 2 apps"]);
    }

    #[test]
    fn suggestions_are_the_installed_work_apps() {
        let suggested = suggested_allowed(&InstalledApps::from(&["Google Chrome", "Discord", "Obsidian"], &["MSTeams_8wekyb3d8bbwe"]));
        let ids: Vec<_> = suggested.iter().filter_map(|entry| entry.known.as_deref()).collect();
        assert_eq!(ids, ["chrome", "teams", "obsidian"]);
    }
}
