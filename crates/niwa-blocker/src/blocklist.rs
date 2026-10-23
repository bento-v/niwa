//! Decides whether a running program is on the block list.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::catalog::{self, KnownApp};

/// Anti-cheat executables. These are never closed, whatever the block list
/// says: only the game process itself is.
const ANTI_CHEAT_FILES: &[&str] = &[
    "vgc.exe",
    "vgtray.exe",
    "easyanticheat.exe",
    "easyanticheat_eos.exe",
    "easyanticheat_setup.exe",
    "easyanticheat_eos_setup.exe",
    "beservice.exe",
    "beservice_x64.exe",
];

/// Folder names that hold anti-cheat components, inside game folders or under
/// Program Files.
const ANTI_CHEAT_DIRS: &[&str] = &[
    "easyanticheat",
    "easyanticheat_eos",
    "battleye",
    "riot vanguard",
];

/// Windows processes that must keep running for the desktop to work.
const SYSTEM_FILES: &[&str] = &[
    "csrss.exe",
    "dwm.exe",
    "explorer.exe",
    "lsass.exe",
    "services.exe",
    "smss.exe",
    "svchost.exe",
    "wininit.exe",
    "winlogon.exe",
];

/// One installed game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    pub name: String,
    pub install_dir: PathBuf,
}

/// The games one launcher has installed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameLibrary {
    /// The launcher's name, such as "Steam".
    pub source: String,
    /// Folders where every subfolder is a game, such as Steam's
    /// `steamapps\common`. Covers games installed after the scan.
    pub roots: Vec<PathBuf>,
    /// Subfolders of a root that aren't games.
    pub ignored_folders: Vec<String>,
    pub games: Vec<Game>,
}

/// A game launcher. Launchers are never closed (Oct 5, 2026), and an
/// allow-only list leaves them open too; only their games are blocked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Launcher {
    pub name: String,
    /// Folders the launcher's own programs run from.
    pub dirs: Vec<PathBuf>,
    /// Single executables, for launchers whose folder also holds other things.
    pub exes: Vec<PathBuf>,
}

impl Launcher {
    /// Whether a program (normalized path) is one of the launcher's own.
    fn runs(&self, image: &str) -> bool {
        self.exes.iter().any(|exe| image == normalize(exe)) || self.dirs.iter().any(|dir| is_under(image, &normalize(dir)))
    }
}

/// What a program's version resource says it is. Survives the file being
/// renamed or moved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIdentity {
    pub product_name: String,
    pub original_filename: String,
}

/// One app the user chose to block, or to allow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppRule {
    /// An app Niwa knows by name, wherever it's installed.
    Known(&'static KnownApp),
    /// The executable at exactly this path.
    Path(PathBuf),
    /// Any executable with this file name. Prototype only.
    FileName(String),
    /// An app picked in the UI: matched by its path, or by its version
    /// resource if it has been moved or renamed.
    Picked {
        name: String,
        path: PathBuf,
        identity: Option<FileIdentity>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct BlockList {
    pub libraries: Vec<GameLibrary>,
    pub apps: Vec<AppRule>,
    /// Apps from the categories the list blocks.
    pub known: Vec<&'static KnownApp>,
    /// Executables or folders that stay allowed even when a rule matches.
    pub exceptions: Vec<PathBuf>,
    /// Set for an allow-only list, which replaces every rule above but the
    /// exceptions.
    pub allow_only: Option<AllowOnly>,
}

/// An allow-only list: every program is a candidate for closing unless it is
/// part of Windows, part of Niwa, or allowed.
///
/// A candidate is only closed while it shows a window on the taskbar, which
/// the caller checks (`Blocked::needs_window`): tray apps, helpers and
/// background services are never touched.
#[derive(Debug, Clone, Default)]
pub struct AllowOnly {
    pub allowed: Vec<AppRule>,
    /// Folders whose programs always stay: Niwa's own.
    pub always: Vec<PathBuf>,
    /// Game launchers, which always stay too (their games don't).
    pub launchers: Vec<Launcher>,
    /// The Windows folder, usually `C:\Windows`.
    pub system_root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockedKind {
    Game,
    App,
}

/// Why a program is blocked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blocked {
    /// A name to show the user, such as "Geometry Dash" or "Notepad".
    pub label: String,
    pub kind: BlockedKind,
    /// True when it only came up because an allow-only list doesn't allow it:
    /// it may be closed only while it has a taskbar window, and only if it
    /// belongs to the user.
    pub needs_window: bool,
}

/// Reads a program's version resource at most once, and only if asked.
struct LazyIdentity<F> {
    read: Option<F>,
    value: Option<FileIdentity>,
}

impl<F: FnOnce() -> Option<FileIdentity>> LazyIdentity<F> {
    fn new(read: F) -> Self {
        LazyIdentity {
            read: Some(read),
            value: None,
        }
    }

    fn get(&mut self) -> Option<&FileIdentity> {
        if let Some(read) = self.read.take() {
            self.value = read();
        }
        self.value.as_ref()
    }
}

impl BlockList {
    /// Checks a running program's full executable path against the list.
    ///
    /// `identity` reads the program's version resource. It is only called
    /// when a picked app could match by identity, since it touches the disk.
    pub fn check(
        &self,
        image_path: &Path,
        identity: impl FnOnce() -> Option<FileIdentity>,
    ) -> Option<Blocked> {
        if is_protected(image_path) {
            return None;
        }
        let image = normalize(image_path);
        if self
            .exceptions
            .iter()
            .any(|allowed| is_same_or_under(&image, &normalize(allowed)))
        {
            return None;
        }
        let mut identity = LazyIdentity::new(identity);

        if let Some(allow_only) = &self.allow_only {
            return allow_only.check(image_path, &image, &mut identity);
        }
        let app = |label: String| Blocked {
            label,
            kind: BlockedKind::App,
            needs_window: false,
        };
        if let Some(label) = matching_rule(&self.apps, image_path, &image, &mut identity) {
            return Some(app(label));
        }
        if let Some(known) = self.known.iter().find(|known| known.matches(&image, || identity.get())) {
            return Some(app(known.name.to_owned()));
        }
        self.check_games(image_path, &image)
    }

    fn check_games(&self, image_path: &Path, image: &str) -> Option<Blocked> {
        let game = |label: String| Blocked {
            label,
            kind: BlockedKind::Game,
            needs_window: false,
        };
        for library in &self.libraries {
            if let Some(found) = library
                .games
                .iter()
                .find(|game| is_under(image, &normalize(&game.install_dir)))
            {
                return Some(game(found.name.clone()));
            }
            for root in library.roots.iter().map(|root| normalize(root)) {
                if !is_under(image, &root) {
                    continue;
                }
                let folder = image[root.len() + 1..].split('\\').next()?;
                if library
                    .ignored_folders
                    .iter()
                    .any(|ignored| ignored.to_lowercase() == folder)
                {
                    return None;
                }
                return Some(game(folder_as_written(image_path, root.len() + 1, folder)));
            }
        }
        None
    }
}

impl AllowOnly {
    fn check<F: FnOnce() -> Option<FileIdentity>>(
        &self,
        image_path: &Path,
        image: &str,
        identity: &mut LazyIdentity<F>,
    ) -> Option<Blocked> {
        if is_part_of_windows(image, &normalize(&self.system_root))
            || self
                .always
                .iter()
                .any(|folder| is_same_or_under(image, &normalize(folder)))
            || self.launchers.iter().any(|launcher| launcher.runs(image))
            || matching_rule(&self.allowed, image_path, image, identity).is_some()
        {
            return None;
        }
        // Name it the way the user would: Niwa's name for it, its product
        // name, or failing those its file name.
        let label = match catalog::every_app().find(|known| known.matches(image, || identity.get())) {
            Some(known) => known.name.to_owned(),
            None => identity
                .get()
                .map(|identity| identity.product_name.trim().to_owned())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| stem(image_path)),
        };
        Some(Blocked {
            label,
            kind: BlockedKind::App,
            needs_window: true,
        })
    }
}

/// The label of the first rule a program matches: by path first, then known
/// apps, then picked apps that have since been moved or renamed. The version
/// resource is only read when a rule needs it.
fn matching_rule<F: FnOnce() -> Option<FileIdentity>>(
    rules: &[AppRule],
    image_path: &Path,
    image: &str,
    identity: &mut LazyIdentity<F>,
) -> Option<String> {
    for rule in rules {
        let hit = match rule {
            AppRule::Path(path) => image == normalize(path),
            AppRule::FileName(name) => file_name(image) == name.to_lowercase(),
            AppRule::Picked { path, .. } => image == normalize(path),
            AppRule::Known(_) => false,
        };
        if hit {
            return Some(rule_label(rule, image_path));
        }
    }
    for rule in rules {
        if let AppRule::Known(known) = rule {
            if known.matches(image, || identity.get()) {
                return Some(known.name.to_owned());
            }
        }
    }

    // Nothing matched by path; a picked app may have been moved or renamed.
    let picked: Vec<(&String, &FileIdentity)> = rules
        .iter()
        .filter_map(|rule| match rule {
            AppRule::Picked {
                name,
                identity: Some(identity),
                ..
            } => Some((name, identity)),
            _ => None,
        })
        .collect();
    if picked.is_empty() {
        return None;
    }
    let actual = identity.get()?;
    picked
        .into_iter()
        .find(|(_, wanted)| same_identity(wanted, actual))
        .map(|(name, _)| name.clone())
}

/// Store packages from Microsoft that are games, not part of Windows.
const MICROSOFT_GAMES: &[&str] = &[
    "microsoft.xbox",
    "microsoft.gamingapp",
    "microsoft.minecraft",
    "microsoft.microsoftsolitaire",
    "microsoft.microsoftmahjong",
    "microsoft.microsoftjigsaw",
    "microsoft.microsoftsudoku",
];

/// True for programs that come with Windows: anything in the Windows folder,
/// and Microsoft's own Store apps (Terminal, Notepad, Calculator, Snipping
/// Tool...) apart from its games. An allow-only list never closes these.
fn is_part_of_windows(image: &str, system_root: &str) -> bool {
    if !system_root.is_empty() && is_under(image, system_root) {
        return true;
    }
    let mut components = image.split('\\');
    while let Some(component) = components.next() {
        if component == "windowsapps" {
            return components.next().is_some_and(|package| {
                (package.starts_with("microsoft.") || package.starts_with("microsoftwindows."))
                    && !MICROSOFT_GAMES.iter().any(|game| package.starts_with(game))
            });
        }
    }
    false
}

/// True for anti-cheat and essential Windows processes, which Niwa never
/// closes.
pub fn is_protected(image_path: &Path) -> bool {
    let image = normalize(image_path);
    let name = file_name(&image);
    ANTI_CHEAT_FILES.contains(&name)
        || SYSTEM_FILES.contains(&name)
        || image
            .split('\\')
            .any(|component| ANTI_CHEAT_DIRS.contains(&component))
}

/// Lowercases a path, uses backslashes throughout and drops any `\\?\` prefix
/// and trailing separator, so paths can be compared as strings.
fn normalize(path: &Path) -> String {
    let text = path.to_string_lossy().replace('/', "\\").to_lowercase();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    text.trim_end_matches('\\').to_owned()
}

fn file_name(normalized: &str) -> &str {
    normalized.rsplit('\\').next().unwrap_or(normalized)
}

fn is_under(normalized: &str, dir: &str) -> bool {
    normalized.len() > dir.len()
        && normalized.starts_with(dir)
        && normalized.as_bytes()[dir.len()] == b'\\'
}

fn is_same_or_under(normalized: &str, target: &str) -> bool {
    normalized == target || is_under(normalized, target)
}

fn same_identity(a: &FileIdentity, b: &FileIdentity) -> bool {
    !a.product_name.is_empty()
        && !a.original_filename.is_empty()
        && a.product_name.eq_ignore_ascii_case(&b.product_name)
        && a.original_filename.eq_ignore_ascii_case(&b.original_filename)
}

fn rule_label(rule: &AppRule, image_path: &Path) -> String {
    match rule {
        AppRule::Picked { name, .. } => name.clone(),
        AppRule::Known(known) => known.name.to_owned(),
        AppRule::Path(_) | AppRule::FileName(_) => stem(image_path),
    }
}

fn stem(image_path: &Path) -> String {
    image_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| image_path.to_string_lossy().into_owned())
}

/// The folder name starting at byte `offset` of the normalized path, in the
/// case the path was given in. Falls back to the lowercased `folder` if
/// lowercasing shifted the offsets (some non-ASCII names).
fn folder_as_written(image_path: &Path, offset: usize, folder: &str) -> String {
    let text = image_path.to_string_lossy().replace('/', "\\");
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    text.get(offset..)
        .and_then(|rest| rest.split('\\').next())
        .filter(|written| written.to_lowercase() == folder)
        .unwrap_or(folder)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steam() -> GameLibrary {
        GameLibrary {
            source: "Steam".into(),
            roots: vec![
                PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common"),
                PathBuf::from(r"D:\SteamLibrary\steamapps\common"),
            ],
            ignored_folders: vec!["Steamworks Shared".into()],
            games: vec![Game {
                name: "Geometry Dash".into(),
                install_dir: PathBuf::from(
                    r"C:\Program Files (x86)\Steam\steamapps\common\Geometry Dash",
                ),
            }],
        }
    }

    fn steam_list() -> BlockList {
        BlockList {
            libraries: vec![steam()],
            ..BlockList::default()
        }
    }

    fn label(list: &BlockList, path: &str) -> Option<String> {
        list.check(Path::new(path), || None).map(|blocked| blocked.label)
    }

    #[test]
    fn steam_game_is_blocked_by_its_folder() {
        let blocked = steam_list()
            .check(
                Path::new(
                    r"C:\Program Files (x86)\Steam\steamapps\common\Geometry Dash\GeometryDash.exe",
                ),
                || None,
            )
            .unwrap();
        assert_eq!(blocked.label, "Geometry Dash");
        assert_eq!(blocked.kind, BlockedKind::Game);
    }

    #[test]
    fn renaming_the_exe_does_not_get_around_it() {
        assert_eq!(
            label(
                &steam_list(),
                r"c:\program files (x86)\steam\steamapps\common\geometry dash\homework.exe"
            ),
            Some("Geometry Dash".into())
        );
    }

    #[test]
    fn game_without_a_manifest_is_named_after_its_folder() {
        assert_eq!(
            label(
                &steam_list(),
                r"D:\SteamLibrary\steamapps\common\PEAK\bin\PEAK.exe"
            ),
            Some("PEAK".into())
        );
    }

    #[test]
    fn programs_outside_the_libraries_are_left_alone() {
        let list = steam_list();
        assert_eq!(label(&list, r"C:\Program Files (x86)\Steam\steam.exe"), None);
        assert_eq!(label(&list, r"C:\Windows\System32\notepad.exe"), None);
        // A sibling folder that merely starts with the same text.
        assert_eq!(
            label(&list, r"D:\SteamLibrary\steamapps\common2\Game\game.exe"),
            None
        );
    }

    #[test]
    fn ignored_folders_are_not_blocked() {
        assert_eq!(
            label(
                &steam_list(),
                r"C:\Program Files (x86)\Steam\steamapps\common\Steamworks Shared\_CommonRedist\vcredist\vc_redist.x64.exe"
            ),
            None
        );
    }

    #[test]
    fn games_listed_without_a_root_are_blocked() {
        let list = BlockList {
            libraries: vec![GameLibrary {
                source: "Riot".into(),
                games: vec![Game {
                    name: "VALORANT".into(),
                    install_dir: PathBuf::from(r"C:\Riot Games\VALORANT\live"),
                }],
                ..GameLibrary::default()
            }],
            ..BlockList::default()
        };
        assert_eq!(
            label(
                &list,
                r"C:\Riot Games\VALORANT\live\ShooterGame\Binaries\Win64\VALORANT-Win64-Shipping.exe"
            ),
            Some("VALORANT".into())
        );
        assert_eq!(
            label(&list, r"C:\Riot Games\Riot Client\RiotClientServices.exe"),
            None
        );
    }

    #[test]
    fn launchers_are_never_blocked() {
        let list = steam_list();
        assert_eq!(label(&list, r"C:\Program Files (x86)\Steam\steam.exe"), None);
        assert_eq!(label(&list, r"C:\Riot Games\Riot Client\RiotClientElectron\Riot Client.exe"), None);
    }

    #[test]
    fn anti_cheat_is_never_blocked() {
        let mut list = steam_list();
        list.apps.push(AppRule::FileName("vgc.exe".into()));
        for path in [
            r"C:\Program Files\Riot Vanguard\vgc.exe",
            r"C:\Program Files\Riot Vanguard\vgtray.exe",
            r"D:\SteamLibrary\steamapps\common\Game\EasyAntiCheat\EasyAntiCheat_EOS_Setup.exe",
            r"D:\SteamLibrary\steamapps\common\Game\BattlEye\BEService_x64.exe",
            r"C:\Program Files (x86)\Common Files\BattlEye\BEService.exe",
        ] {
            assert_eq!(label(&list, path), None, "{path}");
        }
    }

    #[test]
    fn system_processes_are_never_blocked() {
        let list = BlockList {
            apps: vec![AppRule::FileName("explorer.exe".into())],
            ..BlockList::default()
        };
        assert_eq!(label(&list, r"C:\Windows\explorer.exe"), None);
    }

    #[test]
    fn exceptions_allow_an_app_inside_a_blocked_category() {
        let mut list = steam_list();
        list.exceptions.push(PathBuf::from(
            r"D:\SteamLibrary\steamapps\common\wallpaper_engine",
        ));
        assert_eq!(
            label(
                &list,
                r"D:\SteamLibrary\steamapps\common\wallpaper_engine\wallpaper64.exe"
            ),
            None
        );
        assert_eq!(
            label(&list, r"D:\SteamLibrary\steamapps\common\PEAK\PEAK.exe"),
            Some("PEAK".into())
        );
    }

    #[test]
    fn app_rules_match_by_name_or_exact_path() {
        let list = BlockList {
            apps: vec![
                AppRule::FileName("Notepad.exe".into()),
                AppRule::Path(PathBuf::from(r"C:\Apps\Discord\Discord.exe")),
            ],
            ..BlockList::default()
        };
        assert_eq!(
            label(
                &list,
                r"C:\Program Files\WindowsApps\Microsoft.WindowsNotepad_11\Notepad\Notepad.exe"
            ),
            Some("Notepad".into())
        );
        assert_eq!(
            label(&list, r"c:\apps\discord\discord.exe"),
            Some("discord".into())
        );
        assert_eq!(label(&list, r"C:\Other\Discord.exe"), None);
    }

    fn discord() -> FileIdentity {
        FileIdentity {
            product_name: "Discord".into(),
            original_filename: "Discord.exe".into(),
        }
    }

    fn picked_discord() -> BlockList {
        BlockList {
            apps: vec![AppRule::Picked {
                name: "Discord".into(),
                path: PathBuf::from(r"C:\Apps\Discord\app-1.0\Discord.exe"),
                identity: Some(discord()),
            }],
            ..BlockList::default()
        }
    }

    #[test]
    fn picked_app_matches_by_path_without_reading_the_file() {
        let blocked = picked_discord()
            .check(Path::new(r"C:\Apps\Discord\app-1.0\Discord.exe"), || {
                panic!("identity should not be read when the path matches")
            })
            .unwrap();
        assert_eq!(blocked.label, "Discord");
        assert_eq!(blocked.kind, BlockedKind::App);
    }

    #[test]
    fn picked_app_still_matches_after_an_update_or_rename() {
        let list = picked_discord();
        let moved = Path::new(r"C:\Apps\Discord\app-1.1\homework.exe");
        assert_eq!(
            list.check(moved, || Some(discord())).map(|b| b.label),
            Some("Discord".into())
        );
        let other = FileIdentity {
            product_name: "Slack".into(),
            original_filename: "slack.exe".into(),
        };
        assert_eq!(list.check(moved, || Some(other)), None);
        assert_eq!(list.check(moved, || None), None);
    }

    #[test]
    fn blank_identities_never_match() {
        let blank = FileIdentity {
            product_name: String::new(),
            original_filename: String::new(),
        };
        let list = BlockList {
            apps: vec![AppRule::Picked {
                name: "Tool".into(),
                path: PathBuf::from(r"C:\Tools\tool.exe"),
                identity: Some(blank.clone()),
            }],
            ..BlockList::default()
        };
        assert_eq!(list.check(Path::new(r"C:\Other\thing.exe"), || Some(blank)), None);
    }

    #[test]
    fn identity_is_not_read_when_no_app_was_picked() {
        let list = steam_list();
        assert_eq!(
            list.check(Path::new(r"C:\Windows\System32\notepad.exe"), || {
                panic!("identity should not be read")
            }),
            None
        );
    }

    #[test]
    fn extended_length_prefix_is_ignored() {
        assert_eq!(
            label(
                &steam_list(),
                r"\\?\D:\SteamLibrary\steamapps\common\PEAK\PEAK.exe"
            ),
            Some("PEAK".into())
        );
    }

    fn allow_only(allowed: Vec<AppRule>) -> BlockList {
        BlockList {
            allow_only: Some(AllowOnly {
                allowed,
                always: vec![PathBuf::from(r"C:\Program Files\Niwa")],
                launchers: Vec::new(),
                system_root: PathBuf::from(r"C:\Windows"),
            }),
            ..BlockList::default()
        }
    }

    fn product(name: &str) -> FileIdentity {
        FileIdentity {
            product_name: name.into(),
            original_filename: String::new(),
        }
    }

    #[test]
    fn allow_only_closes_what_is_not_allowed_once_it_has_a_window() {
        let list = allow_only(vec![AppRule::Known(crate::catalog::known("chrome").unwrap())]);
        let blocked = list
            .check(Path::new(r"C:\Users\a\AppData\Local\Discord\app-1.0\Discord.exe"), || {
                Some(product("Discord"))
            })
            .unwrap();
        assert_eq!(blocked.label, "Discord");
        assert!(blocked.needs_window);
        // Something Niwa doesn't know is named after its product, then its file.
        let tool = list.check(Path::new(r"D:\Tools\thing.exe"), || Some(product("Thing Pro"))).unwrap();
        assert_eq!(tool.label, "Thing Pro");
        let bare = list.check(Path::new(r"D:\Tools\thing.exe"), || None).unwrap();
        assert_eq!(bare.label, "thing");
    }

    #[test]
    fn allow_only_keeps_allowed_apps() {
        let list = allow_only(vec![
            AppRule::Known(crate::catalog::known("chrome").unwrap()),
            AppRule::Picked {
                name: "Editor".into(),
                path: PathBuf::from(r"C:\Tools\editor.exe"),
                identity: None,
            },
        ]);
        let chrome = Path::new(r"C:\Program Files\Google\Chrome\Application\chrome.exe");
        assert_eq!(list.check(chrome, || Some(product("Google Chrome"))), None);
        assert_eq!(list.check(Path::new(r"C:\Tools\editor.exe"), || None), None);
        // A program merely named chrome.exe isn't Chrome.
        assert!(list.check(Path::new(r"D:\Games\chrome.exe"), || Some(product("Some Game"))).is_some());
    }

    #[test]
    fn allow_only_never_touches_windows_or_niwa() {
        let list = allow_only(Vec::new());
        for path in [
            r"C:\Windows\explorer.exe",
            r"C:\Windows\System32\Taskmgr.exe",
            r"C:\Windows\ImmersiveControlPanel\SystemSettings.exe",
            r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_1.21.0.0_x64__8wekyb3d8bbwe\WindowsTerminal.exe",
            r"C:\Program Files\WindowsApps\Microsoft.WindowsNotepad_11.0.0.0_x64__8wekyb3d8bbwe\Notepad\Notepad.exe",
            r"C:\Program Files\Niwa\niwa-app.exe",
            r"C:\Riot Games\VALORANT\live\ShooterGame\Binaries\Win64\EasyAntiCheat\EasyAntiCheat.exe",
            r"C:\Program Files\Riot Vanguard\vgc.exe",
        ] {
            assert_eq!(list.check(Path::new(path), || None), None, "{path}");
        }
    }

    #[test]
    fn microsoft_games_are_not_part_of_windows() {
        let list = allow_only(Vec::new());
        let solitaire = r"C:\Program Files\WindowsApps\Microsoft.MicrosoftSolitaireCollection_4.0.0.0_x64__8wekyb3d8bbwe\Solitaire.exe";
        assert!(list.check(Path::new(solitaire), || None).is_some());
        let xbox = r"C:\Program Files\WindowsApps\Microsoft.GamingApp_2.0.0.0_x64__8wekyb3d8bbwe\XboxPcApp.exe";
        assert!(list.check(Path::new(xbox), || None).is_some());
    }

    #[test]
    fn exceptions_still_hold_in_allow_only() {
        let mut list = allow_only(Vec::new());
        list.exceptions.push(PathBuf::from(r"C:\Tools"));
        assert_eq!(list.check(Path::new(r"C:\Tools\editor.exe"), || None), None);
    }

    #[test]
    fn categories_block_their_known_apps() {
        let list = BlockList {
            known: crate::catalog::category("chat").unwrap().apps.iter().collect(),
            ..BlockList::default()
        };
        let discord = Path::new(r"C:\Users\a\AppData\Local\Discord\app-1.0\Discord.exe");
        let blocked = list.check(discord, || Some(product("Discord"))).unwrap();
        assert_eq!(blocked.label, "Discord");
        assert!(!blocked.needs_window);
        // Same file name, different product: left alone.
        assert_eq!(list.check(Path::new(r"C:\Mods\Discord.exe"), || Some(product("Mod Tool"))), None);
        // Programs no category names are left alone without reading them.
        assert_eq!(
            list.check(Path::new(r"C:\Tools\editor.exe"), || panic!("identity read")),
            None
        );
    }
}
