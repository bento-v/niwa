//! Apps Niwa knows by name: the ready-made categories a list can block, and
//! the work apps it suggests for an allow-only list.
//!
//! A known app is recognised by its version resource's product name together
//! with its program's file name, or by the Microsoft Store package it runs
//! from. Never by file name alone.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::blocklist::FileIdentity;

#[derive(Debug, PartialEq, Eq)]
pub struct KnownApp {
    pub id: &'static str,
    pub name: &'static str,
    /// The product names its version resource may give.
    pub products: &'static [&'static str],
    /// Its programs' file names. They only count alongside a product name.
    pub exes: &'static [&'static str],
    /// The Microsoft Store package it's installed as, if it is one: it runs
    /// from `WindowsApps\<package>_<version>_...`.
    pub package: Option<&'static str>,
    /// How it starts in Settings > Apps > Installed apps, to tell whether it's installed.
    pub listed_as: &'static [&'static str],
}

impl KnownApp {
    /// Whether a running program is this app. `image` is the normalized
    /// (lowercase, backslashed) path; `identity` reads the version resource
    /// and is only called when the file name already fits.
    pub fn matches<'a>(&self, image: &str, identity: impl FnOnce() -> Option<&'a FileIdentity>) -> bool {
        if self.package.is_some_and(|package| in_package(image, package)) {
            return true;
        }
        let name = image.rsplit('\\').next().unwrap_or(image);
        if !self.exes.iter().any(|exe| exe.eq_ignore_ascii_case(name)) {
            return false;
        }
        identity().is_some_and(|identity| {
            self.products
                .iter()
                .any(|product| product.eq_ignore_ascii_case(identity.product_name.trim()))
        })
    }
}

/// A group of apps that can be blocked with one switch.
#[derive(Debug, PartialEq, Eq)]
pub struct Category {
    pub id: &'static str,
    pub label: &'static str,
    pub apps: &'static [KnownApp],
}

const DISCORD: KnownApp = KnownApp {
    id: "discord",
    name: "Discord",
    products: &["Discord", "Discord PTB", "Discord Canary"],
    exes: &["Discord.exe", "DiscordPTB.exe", "DiscordCanary.exe"],
    package: None,
    listed_as: &["Discord"],
};
const TELEGRAM: KnownApp = KnownApp {
    id: "telegram",
    name: "Telegram",
    products: &["Telegram Desktop"],
    exes: &["Telegram.exe"],
    package: Some("TelegramMessengerLLP.TelegramDesktop"),
    listed_as: &["Telegram Desktop"],
};
const WHATSAPP: KnownApp = KnownApp {
    id: "whatsapp",
    name: "WhatsApp",
    products: &["WhatsApp"],
    exes: &["WhatsApp.exe"],
    package: Some("5319275A.WhatsAppDesktop"),
    listed_as: &["WhatsApp"],
};
const SIGNAL: KnownApp = KnownApp {
    id: "signal",
    name: "Signal",
    products: &["Signal"],
    exes: &["Signal.exe"],
    package: None,
    listed_as: &["Signal"],
};
const MESSENGER: KnownApp = KnownApp {
    id: "messenger",
    name: "Messenger",
    products: &["Messenger"],
    exes: &["Messenger.exe"],
    package: Some("FACEBOOK.317180B0BB486"),
    listed_as: &["Messenger"],
};
const INSTAGRAM: KnownApp = KnownApp {
    id: "instagram",
    name: "Instagram",
    products: &[],
    exes: &[],
    package: Some("Facebook.InstagramBeta"),
    listed_as: &[],
};
const TIKTOK: KnownApp = KnownApp {
    id: "tiktok",
    name: "TikTok",
    products: &[],
    exes: &[],
    package: Some("BytedancePte.Ltd.TikTok"),
    listed_as: &[],
};

const SPOTIFY: KnownApp = KnownApp {
    id: "spotify",
    name: "Spotify",
    products: &["Spotify"],
    exes: &["Spotify.exe"],
    package: Some("SpotifyAB.SpotifyMusic"),
    listed_as: &["Spotify"],
};
const NETFLIX: KnownApp = KnownApp {
    id: "netflix",
    name: "Netflix",
    products: &[],
    exes: &[],
    package: Some("4DF9E0F8.Netflix"),
    listed_as: &[],
};
const PRIME_VIDEO: KnownApp = KnownApp {
    id: "prime-video",
    name: "Prime Video",
    products: &[],
    exes: &[],
    package: Some("AmazonVideo.PrimeVideo"),
    listed_as: &[],
};
const DISNEY_PLUS: KnownApp = KnownApp {
    id: "disney-plus",
    name: "Disney+",
    products: &[],
    exes: &[],
    package: Some("Disney.37853FC22B2CE"),
    listed_as: &[],
};
const VLC: KnownApp = KnownApp {
    id: "vlc",
    name: "VLC",
    products: &["VLC media player"],
    exes: &["vlc.exe"],
    package: None,
    listed_as: &["VLC media player"],
};

const CHROME: KnownApp = KnownApp {
    id: "chrome",
    name: "Google Chrome",
    products: &["Google Chrome"],
    exes: &["chrome.exe"],
    package: None,
    listed_as: &["Google Chrome"],
};
const EDGE: KnownApp = KnownApp {
    id: "edge",
    name: "Microsoft Edge",
    products: &["Microsoft Edge"],
    exes: &["msedge.exe"],
    package: None,
    listed_as: &["Microsoft Edge"],
};
const FIREFOX: KnownApp = KnownApp {
    id: "firefox",
    name: "Firefox",
    products: &["Firefox"],
    exes: &["firefox.exe"],
    package: None,
    listed_as: &["Mozilla Firefox"],
};
const OPERA: KnownApp = KnownApp {
    id: "opera",
    name: "Opera",
    products: &["Opera Internet Browser", "Opera GX Internet Browser"],
    exes: &["opera.exe"],
    package: None,
    listed_as: &["Opera"],
};
const BRAVE: KnownApp = KnownApp {
    id: "brave",
    name: "Brave",
    products: &["Brave Browser"],
    exes: &["brave.exe"],
    package: None,
    listed_as: &["Brave"],
};

const VS_CODE: KnownApp = KnownApp {
    id: "vscode",
    name: "VS Code",
    products: &["Visual Studio Code"],
    exes: &["Code.exe"],
    package: None,
    listed_as: &["Microsoft Visual Studio Code"],
};
const OFFICE: KnownApp = KnownApp {
    id: "office",
    name: "Microsoft Office",
    products: &["Microsoft Office", "Microsoft 365", "Microsoft® Office"],
    exes: &["WINWORD.EXE", "EXCEL.EXE", "POWERPNT.EXE", "OUTLOOK.EXE", "ONENOTE.EXE"],
    package: None,
    listed_as: &["Microsoft 365", "Microsoft Office"],
};
const TEAMS: KnownApp = KnownApp {
    id: "teams",
    name: "Microsoft Teams",
    products: &["Microsoft Teams"],
    exes: &["ms-teams.exe"],
    package: Some("MSTeams"),
    listed_as: &[],
};
const SLACK: KnownApp = KnownApp {
    id: "slack",
    name: "Slack",
    products: &["Slack"],
    exes: &["slack.exe"],
    package: Some("91750D7E.Slack"),
    listed_as: &["Slack"],
};
const ZOOM: KnownApp = KnownApp {
    id: "zoom",
    name: "Zoom",
    products: &["Zoom", "Zoom Workplace"],
    exes: &["Zoom.exe"],
    package: None,
    listed_as: &["Zoom"],
};
const NOTION: KnownApp = KnownApp {
    id: "notion",
    name: "Notion",
    products: &["Notion"],
    exes: &["Notion.exe"],
    package: None,
    listed_as: &["Notion"],
};
const OBSIDIAN: KnownApp = KnownApp {
    id: "obsidian",
    name: "Obsidian",
    products: &["Obsidian"],
    exes: &["Obsidian.exe"],
    package: None,
    listed_as: &["Obsidian"],
};
const CLAUDE: KnownApp = KnownApp {
    id: "claude",
    name: "Claude",
    products: &["Claude"],
    exes: &["claude.exe"],
    package: None,
    listed_as: &["Claude"],
};
const ACROBAT: KnownApp = KnownApp {
    id: "acrobat",
    name: "Adobe Acrobat",
    products: &["Adobe Acrobat", "Adobe Acrobat Reader"],
    exes: &["Acrobat.exe", "AcroRd32.exe"],
    package: None,
    listed_as: &["Adobe Acrobat"],
};

/// The ready-made categories on the Blocks screen.
pub const CATEGORIES: &[Category] = &[
    Category {
        id: "chat",
        label: "Chat & social",
        apps: &[DISCORD, TELEGRAM, WHATSAPP, SIGNAL, MESSENGER, INSTAGRAM, TIKTOK],
    },
    Category {
        id: "media",
        label: "Music & video",
        apps: &[SPOTIFY, NETFLIX, PRIME_VIDEO, DISNEY_PLUS, VLC],
    },
    Category {
        id: "browsers",
        label: "Web browsers",
        apps: &[CHROME, EDGE, FIREFOX, OPERA, BRAVE],
    },
];

/// Apps an allow-only list suggests keeping, when they're installed.
pub const WORK_APPS: &[KnownApp] = &[
    CHROME, EDGE, FIREFOX, OPERA, BRAVE, VS_CODE, OFFICE, TEAMS, SLACK, ZOOM, NOTION, OBSIDIAN, CLAUDE,
    ACROBAT,
];

pub fn category(id: &str) -> Option<&'static Category> {
    CATEGORIES.iter().find(|category| category.id == id)
}

/// Any known app by id, from the categories or the work apps.
pub fn known(id: &str) -> Option<&'static KnownApp> {
    every_app().find(|app| app.id == id)
}

/// Every known app. The browsers appear in both lists; that's harmless.
pub fn every_app() -> impl Iterator<Item = &'static KnownApp> {
    CATEGORIES
        .iter()
        .flat_map(|category| category.apps.iter())
        .chain(WORK_APPS.iter())
}

/// True when `image` (normalized) runs from `WindowsApps\<package>_...`.
fn in_package(image: &str, package: &str) -> bool {
    let package = package.to_lowercase();
    let mut components = image.split('\\');
    while let Some(component) = components.next() {
        if component == "windowsapps" {
            return components
                .next()
                .and_then(|folder| folder.strip_prefix(&package))
                .is_some_and(|rest| rest.starts_with('_'));
        }
    }
    false
}

/// Which apps are installed, from Settings' list of installed apps and the
/// user's Microsoft Store packages.
#[derive(Debug, Default, Clone)]
pub struct InstalledApps {
    /// Display names, lowercased.
    names: Vec<String>,
    /// Store package folder names, lowercased.
    packages: HashSet<String>,
}

impl InstalledApps {
    pub fn scan() -> InstalledApps {
        let mut names = Vec::new();
        for (root, path) in [
            (windows_registry::CURRENT_USER, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
            (windows_registry::LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
            (windows_registry::LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
        ] {
            let Ok(key) = root.open(path) else { continue };
            let Ok(subkeys) = key.keys() else { continue };
            for subkey in subkeys {
                if let Ok(name) = key.open(&subkey).and_then(|app| app.get_string("DisplayName")) {
                    names.push(name.to_lowercase());
                }
            }
        }
        let packages = std::env::var_os("LOCALAPPDATA")
            .map(|local| PathBuf::from(local).join("Packages"))
            .and_then(|dir| std::fs::read_dir(dir).ok())
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_lowercase())
            .collect();
        InstalledApps { names, packages }
    }

    #[cfg(test)]
    pub fn from(names: &[&str], packages: &[&str]) -> InstalledApps {
        InstalledApps {
            names: names.iter().map(|name| name.to_lowercase()).collect(),
            packages: packages.iter().map(|package| package.to_lowercase()).collect(),
        }
    }

    pub fn has(&self, app: &KnownApp) -> bool {
        let listed = app.listed_as.iter().any(|prefix| {
            let prefix = prefix.to_lowercase();
            self.names.iter().any(|name| name.starts_with(&prefix))
        });
        let packaged = app.package.is_some_and(|package| {
            let package = format!("{}_", package.to_lowercase());
            self.packages.iter().any(|folder| folder.starts_with(&package))
        });
        listed || packaged
    }

    /// How many of a category's apps are installed.
    pub fn count(&self, category: &Category) -> usize {
        category.apps.iter().filter(|app| self.has(app)).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(product: &str) -> FileIdentity {
        FileIdentity {
            product_name: product.into(),
            original_filename: String::new(),
        }
    }

    #[test]
    fn needs_the_product_and_the_file_name_together() {
        let discord = identity("Discord");
        assert!(DISCORD.matches(r"c:\users\a\appdata\local\discord\app-1.0.9260\discord.exe", || Some(&discord)));
        // The right file name with another product's version resource.
        let other = identity("Totally Not Discord");
        assert!(!DISCORD.matches(r"c:\tools\discord.exe", || Some(&other)));
        // No version resource at all.
        assert!(!DISCORD.matches(r"c:\tools\discord.exe", || None));
        // The right product but another program.
        assert!(!DISCORD.matches(r"c:\users\a\appdata\local\discord\update.exe", || Some(&discord)));
    }

    #[test]
    fn identity_is_only_read_when_the_file_name_fits() {
        assert!(!CHROME.matches(r"c:\windows\notepad.exe", || panic!("read the version resource")));
    }

    #[test]
    fn store_apps_match_by_package() {
        let path = r"c:\program files\windowsapps\4df9e0f8.netflix_6.99.5.0_x64__mcm4njqhnhss8\netflix.exe";
        assert!(NETFLIX.matches(path, || None));
        // A package whose name merely starts the same way.
        let lookalike = r"c:\program files\windowsapps\4df9e0f8.netflixplus_1.0.0.0_x64__abc\netflix.exe";
        assert!(!NETFLIX.matches(lookalike, || None));
        assert!(!NETFLIX.matches(r"c:\games\4df9e0f8.netflix_1\netflix.exe", || None));
    }

    #[test]
    fn categories_and_ids_are_unique() {
        let mut ids = HashSet::new();
        for category in CATEGORIES {
            assert!(ids.insert(category.id), "{}", category.id);
        }
        let mut apps = HashSet::new();
        for category in CATEGORIES {
            for app in category.apps {
                assert!(apps.insert(app.id), "{}", app.id);
            }
        }
        assert!(known("vscode").is_some());
        assert!(known("chrome").is_some());
        assert!(category("media").is_some());
    }

    #[test]
    fn installed_apps_are_found_by_listing_or_package() {
        let installed = InstalledApps::from(
            &["Discord", "Mozilla Firefox (x64 en-US)"],
            &["4DF9E0F8.Netflix_mcm4njqhnhss8"],
        );
        assert!(installed.has(&DISCORD));
        assert!(installed.has(&FIREFOX));
        assert!(installed.has(&NETFLIX));
        assert!(!installed.has(&SPOTIFY));
        assert_eq!(installed.count(category("chat").unwrap()), 1);
        assert_eq!(installed.count(category("media").unwrap()), 1);
        assert_eq!(installed.count(category("browsers").unwrap()), 1);
    }
}
