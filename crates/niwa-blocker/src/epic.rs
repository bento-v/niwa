//! Finds installed Epic Games Store games from the launcher's install
//! manifests: one JSON `.item` file per install, in Epic's ProgramData folder.

use std::fs;
use std::path::{Path, PathBuf};

use crate::blocklist::{Game, GameLibrary, Launcher};

/// Reads one `.item` manifest. Returns `None` for anything that isn't an
/// installed game (the Unreal Engine itself is installed the same way).
pub fn parse_manifest(text: &str) -> Option<Game> {
    let manifest: serde_json::Value = serde_json::from_str(text).ok()?;
    let install_dir = manifest.get("InstallLocation")?.as_str()?;
    if install_dir.is_empty() {
        return None;
    }
    if let Some(categories) = manifest.get("AppCategories").and_then(|c| c.as_array()) {
        if !categories.iter().any(|c| c.as_str() == Some("games")) {
            return None;
        }
    }
    let name = manifest
        .get("DisplayName")
        .and_then(|name| name.as_str())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            Path::new(install_dir)
                .file_name()
                .map(|folder| folder.to_string_lossy().into_owned())
        })?;
    Some(Game {
        name,
        install_dir: PathBuf::from(install_dir),
    })
}

/// Reads every manifest in `manifest_dir`. A missing folder means Epic isn't
/// installed and gives an empty library.
pub fn scan(manifest_dir: &Path) -> GameLibrary {
    let mut games: Vec<Game> = fs::read_dir(manifest_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "item"))
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .filter_map(|text| parse_manifest(&text))
        .collect();
    games.sort_by(|a, b| a.name.cmp(&b.name));
    GameLibrary {
        source: "Epic".into(),
        games,
        ..GameLibrary::default()
    }
}

/// Where the Epic launcher keeps its install manifests.
pub fn manifest_dir() -> PathBuf {
    let data_dir = windows_registry::LOCAL_MACHINE
        .open(r"SOFTWARE\WOW6432Node\Epic Games\EpicGamesLauncher")
        .and_then(|key| key.get_string("AppDataPath"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| program_data().join(r"Epic\EpicGamesLauncher\Data"));
    data_dir.join("Manifests")
}

/// The Epic Games Launcher, if it is installed in a standard location.
pub fn launcher() -> Option<Launcher> {
    let dirs: Vec<PathBuf> = ["ProgramFiles(x86)", "ProgramFiles"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|base| PathBuf::from(base).join(r"Epic Games\Launcher"))
        .filter(|dir| dir.is_dir())
        .collect();
    (!dirs.is_empty()).then(|| Launcher {
        name: "Epic Games Launcher".into(),
        dirs,
        exes: Vec::new(),
    })
}

pub(crate) fn program_data() -> PathBuf {
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROCKET_LEAGUE: &str = r#"{
        "FormatVersion": 0,
        "bIsIncompleteInstall": false,
        "LaunchExecutable": "Binaries/Win64/Launcher.exe",
        "AppCategories": ["public", "games", "applications"],
        "DisplayName": "Rocket League®",
        "InstallLocation": "C:\\Program Files\\Epic Games\\rocketleague",
        "AppName": "Sugar"
    }"#;

    #[test]
    fn reads_a_game_manifest() {
        assert_eq!(
            parse_manifest(ROCKET_LEAGUE),
            Some(Game {
                name: "Rocket League®".into(),
                install_dir: PathBuf::from(r"C:\Program Files\Epic Games\rocketleague"),
            })
        );
    }

    #[test]
    fn engines_and_plugins_are_not_games() {
        let engine = r#"{
            "AppCategories": ["public", "engines"],
            "DisplayName": "Unreal Engine",
            "InstallLocation": "C:\\Program Files\\Epic Games\\UE_5.4"
        }"#;
        assert_eq!(parse_manifest(engine), None);
    }

    #[test]
    fn falls_back_to_the_folder_name() {
        let manifest = r#"{ "InstallLocation": "D:\\Games\\FallGuys" }"#;
        assert_eq!(parse_manifest(manifest).unwrap().name, "FallGuys");
    }

    #[test]
    fn rejects_broken_or_empty_manifests() {
        assert_eq!(parse_manifest("not json"), None);
        assert_eq!(parse_manifest(r#"{ "DisplayName": "X" }"#), None);
        assert_eq!(parse_manifest(r#"{ "InstallLocation": "" }"#), None);
    }

    #[test]
    fn scans_a_manifest_folder() {
        let dir = std::env::temp_dir().join(format!("niwa-epic-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("A.item"), ROCKET_LEAGUE).unwrap();
        fs::write(dir.join("B.item"), "broken").unwrap();
        fs::write(dir.join("notes.txt"), ROCKET_LEAGUE).unwrap();

        let library = scan(&dir);
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(library.source, "Epic");
        assert_eq!(library.games.len(), 1);
        assert!(scan(Path::new(r"C:\does\not\exist")).games.is_empty());
    }
}
