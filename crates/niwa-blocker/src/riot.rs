//! Finds installed Riot games from the Riot Client's records in ProgramData:
//! `Metadata\<product>\<product>.product_settings.yaml` for each game, and
//! `RiotClientInstalls.json` for the client itself.

use std::fs;
use std::path::{Path, PathBuf};

use crate::blocklist::{Game, GameLibrary, Launcher};
use crate::epic::program_data;

/// Riot's records folder, `%ProgramData%\Riot Games`.
pub fn data_dir() -> PathBuf {
    program_data().join("Riot Games")
}

/// Reads a game's install folder out of its `product_settings.yaml`.
///
/// The file is flat enough that the two keys needed are read line by line
/// rather than with a YAML parser.
pub fn parse_product_settings(text: &str) -> Option<Game> {
    let value = |key: &str| {
        text.lines().find_map(|line| {
            let rest = line.strip_prefix(key)?.strip_prefix(':')?;
            let path = rest.trim().trim_matches('"').replace('/', "\\");
            (!path.is_empty()).then(|| PathBuf::from(path))
        })
    };
    let install_dir = value("product_install_full_path")?;
    // "C:\Riot Games\VALORANT\live" is named after its folder under the
    // install root, "VALORANT".
    let name = value("product_install_root")
        .and_then(|root| {
            install_dir
                .strip_prefix(root)
                .ok()?
                .components()
                .next()
                .map(|folder| folder.as_os_str().to_string_lossy().into_owned())
        })
        .or_else(|| {
            install_dir
                .file_name()
                .map(|folder| folder.to_string_lossy().into_owned())
        })?;
    Some(Game { name, install_dir })
}

/// Reads every product under `data_dir\Metadata`. A missing folder means no
/// Riot games are installed and gives an empty library.
pub fn scan(data_dir: &Path) -> GameLibrary {
    let mut games: Vec<Game> = fs::read_dir(data_dir.join("Metadata"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|product| fs::read_dir(product.path()).ok())
        .flatten()
        .flatten()
        .filter(|file| {
            file.file_name()
                .to_string_lossy()
                .ends_with(".product_settings.yaml")
        })
        .filter_map(|file| fs::read_to_string(file.path()).ok())
        .filter_map(|text| parse_product_settings(&text))
        // Riot keeps records for games that were uninstalled.
        .filter(|game| game.install_dir.is_dir())
        .collect();
    games.sort_by(|a, b| a.name.cmp(&b.name));
    games.dedup();
    GameLibrary {
        source: "Riot".into(),
        games,
        ..GameLibrary::default()
    }
}

/// The folders the Riot Client runs from, from `RiotClientInstalls.json`.
pub fn parse_client_installs(text: &str) -> Vec<PathBuf> {
    fn collect(value: &serde_json::Value, out: &mut Vec<PathBuf>) {
        match value {
            serde_json::Value::String(path) => {
                let exe = PathBuf::from(path.replace('/', "\\"));
                if let Some(dir) = exe.parent().filter(|_| exe.extension().is_some()) {
                    out.push(dir.to_path_buf());
                }
            }
            serde_json::Value::Object(map) => map.values().for_each(|v| collect(v, out)),
            _ => {}
        }
    }

    let mut dirs = Vec::new();
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
        collect(&json, &mut dirs);
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

/// The Riot Client, if it is installed.
pub fn launcher(data_dir: &Path) -> Option<Launcher> {
    let text = fs::read_to_string(data_dir.join("RiotClientInstalls.json")).ok()?;
    let dirs = parse_client_installs(&text);
    (!dirs.is_empty()).then(|| Launcher {
        name: "Riot Client".into(),
        dirs,
        exes: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALORANT: &str = r#"auto_patching_enabled_by_player: false
dependencies:
    Vanguard: true
patching_policy: "manual"
product_install_full_path: "C:/Riot Games/VALORANT/live"
product_install_root: "C:/Riot Games"
settings:
    create_shortcut: false
shortcut_name: "VALORANT.lnk"
"#;

    #[test]
    fn reads_a_product_settings_file() {
        assert_eq!(
            parse_product_settings(VALORANT),
            Some(Game {
                name: "VALORANT".into(),
                install_dir: PathBuf::from(r"C:\Riot Games\VALORANT\live"),
            })
        );
    }

    #[test]
    fn names_a_game_after_its_folder_when_there_is_no_root() {
        let settings = "product_install_full_path: \"D:/Games/League of Legends\"\n";
        assert_eq!(parse_product_settings(settings).unwrap().name, "League of Legends");
    }

    #[test]
    fn settings_without_an_install_path_are_skipped() {
        assert_eq!(parse_product_settings("patching_policy: \"manual\"\n"), None);
        assert_eq!(parse_product_settings("product_install_full_path: \"\"\n"), None);
    }

    #[test]
    fn reads_the_client_folders() {
        let installs = r#"{
            "associated_client": {
                "C:/Riot Games/VALORANT/live/": "C:/Riot Games/Riot Client/RiotClientServices.exe"
            },
            "rc_default": "C:/Riot Games/Riot Client/RiotClientServices.exe",
            "rc_live": "C:/Riot Games/Riot Client/RiotClientServices.exe"
        }"#;
        assert_eq!(
            parse_client_installs(installs),
            vec![PathBuf::from(r"C:\Riot Games\Riot Client")]
        );
        assert!(parse_client_installs("not json").is_empty());
    }

    #[test]
    fn scans_installed_products_only() {
        let dir = std::env::temp_dir().join(format!("niwa-riot-test-{}", std::process::id()));
        let installed = dir.join("Games").join("VALORANT").join("live");
        fs::create_dir_all(&installed).unwrap();
        let root = dir.join("Games").to_string_lossy().replace('\\', "/");
        for (product, folder) in [("valorant.live", "VALORANT/live"), ("bacon.live", "LoR/live")] {
            let product_dir = dir.join("Metadata").join(product);
            fs::create_dir_all(&product_dir).unwrap();
            fs::write(
                product_dir.join(format!("{product}.product_settings.yaml")),
                format!(
                    "product_install_full_path: \"{root}/{folder}\"\nproduct_install_root: \"{root}\"\n"
                ),
            )
            .unwrap();
        }

        let library = scan(&dir);
        fs::remove_dir_all(&dir).unwrap();

        assert_eq!(library.source, "Riot");
        assert_eq!(
            library.games,
            vec![Game {
                name: "VALORANT".into(),
                install_dir: installed,
            }]
        );
    }
}
