//! Finds installed Steam games from Steam's own library files.
//!
//! `steamapps\libraryfolders.vdf` lists every library folder; each library's
//! `steamapps\appmanifest_<id>.acf` names a game and its folder under
//! `steamapps\common`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::blocklist::{Game, GameLibrary, Launcher};
use crate::vdf;

/// Folders under `steamapps\common` that hold shared redistributables or
/// configuration rather than games.
const NON_GAME_DIRS: &[&str] = &["Steamworks Shared", "Steam Controller Configs"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamGame {
    pub app_id: String,
    pub name: String,
    pub install_dir: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SteamLibraries {
    /// Each library's `steamapps\common` folder. Anything running from inside
    /// one of these counts as a Steam game, including games installed after
    /// the scan.
    pub common_dirs: Vec<PathBuf>,
    pub games: Vec<SteamGame>,
}

impl SteamLibraries {
    pub fn into_library(self) -> GameLibrary {
        GameLibrary {
            source: "Steam".into(),
            roots: self.common_dirs,
            ignored_folders: NON_GAME_DIRS.iter().map(|dir| dir.to_string()).collect(),
            games: self
                .games
                .into_iter()
                .map(|game| Game {
                    name: game.name,
                    install_dir: game.install_dir,
                })
                .collect(),
        }
    }
}

/// The Steam client itself. Only `steam.exe` is listed: its helpers exit with
/// it, and its background service must be left alone.
pub fn launcher(steam_root: &Path) -> Launcher {
    Launcher {
        name: "Steam".into(),
        // Steam's window is drawn by steamwebhelper.exe, under `bin`. Its
        // games are under `steamapps`, which this leaves out.
        dirs: vec![steam_root.join("bin")],
        exes: vec![steam_root.join("steam.exe")],
    }
}

/// True for a folder name under `steamapps\common` that isn't a game.
pub fn is_non_game_dir(folder: &str) -> bool {
    NON_GAME_DIRS
        .iter()
        .any(|dir| dir.eq_ignore_ascii_case(folder))
}

/// Reads the library folder paths out of `libraryfolders.vdf`.
pub fn parse_library_folders(text: &str) -> Result<Vec<PathBuf>, vdf::ParseError> {
    let doc = vdf::parse(text)?;
    let Some(folders) = doc.get("libraryfolders") else {
        return Ok(Vec::new());
    };
    Ok(folders
        .entries()
        .iter()
        .filter_map(|(_, folder)| folder.get("path")?.as_str())
        .map(PathBuf::from)
        .collect())
}

/// Reads one `appmanifest_<id>.acf`. `common_dir` is the library's
/// `steamapps\common` folder, which the manifest's `installdir` is relative to.
pub fn parse_app_manifest(text: &str, common_dir: &Path) -> Option<SteamGame> {
    let doc = vdf::parse(text).ok()?;
    let state = doc.get("AppState")?;
    let install_dir = state.get("installdir")?.as_str()?;
    if is_non_game_dir(install_dir) {
        return None;
    }
    Some(SteamGame {
        app_id: state.get("appid")?.as_str()?.to_owned(),
        name: state.get("name")?.as_str()?.to_owned(),
        install_dir: common_dir.join(install_dir),
    })
}

/// Scans every library of the Steam install at `steam_root`.
///
/// A library or manifest that can't be read is skipped: a drive that is
/// unplugged shouldn't stop the others from being found.
pub fn scan(steam_root: &Path) -> io::Result<SteamLibraries> {
    let text = fs::read_to_string(steam_root.join("steamapps").join("libraryfolders.vdf"))?;
    let folders = parse_library_folders(&text)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut libraries = SteamLibraries::default();
    for folder in folders {
        let steamapps = folder.join("steamapps");
        let common_dir = steamapps.join("common");
        let Ok(entries) = fs::read_dir(&steamapps) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if !(file_name.starts_with("appmanifest_") && file_name.ends_with(".acf")) {
                continue;
            }
            let Ok(text) = fs::read_to_string(entry.path()) else {
                continue;
            };
            if let Some(game) = parse_app_manifest(&text, &common_dir) {
                libraries.games.push(game);
            }
        }
        libraries.common_dirs.push(common_dir);
    }
    libraries.games.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(libraries)
}

/// Where Steam is installed, from the registry, falling back to the default
/// location.
pub fn find_steam_root() -> Option<PathBuf> {
    let from_registry = windows_registry::CURRENT_USER
        .open(r"Software\Valve\Steam")
        .and_then(|key| key.get_string("SteamPath"))
        .or_else(|_| {
            windows_registry::LOCAL_MACHINE
                .open(r"SOFTWARE\WOW6432Node\Valve\Steam")
                .and_then(|key| key.get_string("InstallPath"))
        })
        .ok()
        // HKCU stores the path with forward slashes.
        .map(|path| PathBuf::from(path.replace('/', "\\")));

    from_registry
        .into_iter()
        .chain([PathBuf::from(r"C:\Program Files (x86)\Steam")])
        .find(|root| root.join("steamapps").join("libraryfolders.vdf").is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBRARY_FOLDERS: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
		"apps"
		{
			"228980"		"158365987"
			"322170"		"328394076"
		}
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
		"apps"
		{
		}
	}
}
"#;

    const GEOMETRY_DASH: &str = r#"
"AppState"
{
	"appid"		"322170"
	"universe"		"1"
	"name"		"Geometry Dash"
	"StateFlags"		"4"
	"installdir"		"Geometry Dash"
	"InstalledDepots"
	{
		"322171"
		{
			"manifest"		"3816559102876907245"
		}
	}
}
"#;

    #[test]
    fn reads_every_library_folder() {
        assert_eq!(
            parse_library_folders(LIBRARY_FOLDERS).unwrap(),
            vec![
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"D:\SteamLibrary"),
            ]
        );
    }

    #[test]
    fn missing_libraryfolders_key_is_empty() {
        assert_eq!(parse_library_folders(r#""other" { }"#).unwrap(), Vec::<PathBuf>::new());
    }

    #[test]
    fn reads_an_app_manifest() {
        let common = Path::new(r"D:\SteamLibrary\steamapps\common");
        assert_eq!(
            parse_app_manifest(GEOMETRY_DASH, common),
            Some(SteamGame {
                app_id: "322170".into(),
                name: "Geometry Dash".into(),
                install_dir: PathBuf::from(r"D:\SteamLibrary\steamapps\common\Geometry Dash"),
            })
        );
    }

    #[test]
    fn redistributables_are_not_games() {
        let manifest = r#""AppState" { "appid" "228980" "name" "Steamworks Common Redistributables" "installdir" "Steamworks Shared" }"#;
        assert_eq!(parse_app_manifest(manifest, Path::new(r"C:\x")), None);
    }

    #[test]
    fn scans_a_library_on_disk() {
        let root = std::env::temp_dir().join(format!("niwa-steam-test-{}", std::process::id()));
        let steamapps = root.join("steamapps");
        fs::create_dir_all(steamapps.join("common").join("Geometry Dash")).unwrap();
        let root_escaped = root.to_string_lossy().replace('\\', "\\\\");
        fs::write(
            steamapps.join("libraryfolders.vdf"),
            format!("\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{root_escaped}\"\n\t}}\n}}\n"),
        )
        .unwrap();
        fs::write(steamapps.join("appmanifest_322170.acf"), GEOMETRY_DASH).unwrap();
        fs::write(steamapps.join("appmanifest_bad.acf"), "not a manifest {").unwrap();

        let libraries = scan(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(libraries.common_dirs, vec![steamapps.join("common")]);
        assert_eq!(libraries.games.len(), 1);
        assert_eq!(libraries.games[0].name, "Geometry Dash");
        assert_eq!(
            libraries.games[0].install_dir,
            steamapps.join("common").join("Geometry Dash")
        );
    }
}
