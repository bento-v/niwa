//! Takes Niwa's old block out of the Windows hosts file.
//!
//! Niwa blocked websites through the hosts file until Oct 6, 2026, when
//! website blocking was removed. A block left behind by an earlier version
//! (between a "# BEGIN Niwa" line and a "# END Niwa" line) would keep those
//! sites blocked for good, so the service removes it when it starts. Only
//! the service can (the hosts file needs admin rights); it never adds
//! anything, and every byte outside Niwa's block is kept.

use std::fs;
use std::io::{self, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use windows::core::HSTRING;
use windows::Win32::Storage::FileSystem::{ReplaceFileW, REPLACE_FILE_FLAGS};

const BEGIN: &str = "# BEGIN Niwa";
const END_LINE: &str = "# END Niwa";
/// Stops a console window flashing up when a command-line tool is run.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The hosts file. Debug builds can point it elsewhere with `NIWA_HOSTS`, so
/// tests never touch the real one.
pub fn path() -> PathBuf {
    if let Some(path) = test_path() {
        return path;
    }
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join(r"System32\drivers\etc\hosts")
}

fn test_path() -> Option<PathBuf> {
    cfg!(debug_assertions)
        .then(|| std::env::var_os("NIWA_HOSTS").map(PathBuf::from))
        .flatten()
}

/// `text` without Niwa's block. Everything outside it is kept byte for byte.
fn without_block(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut inside = false;
    for line in text.split_inclusive(|&byte| byte == b'\n') {
        let content = line.strip_suffix(b"\n").unwrap_or(line);
        let content = content.strip_suffix(b"\r").unwrap_or(content);
        if content == END_LINE.as_bytes() {
            inside = false;
            continue;
        }
        if content.starts_with(BEGIN.as_bytes()) {
            inside = true;
            continue;
        }
        // A block whose end line went missing ends at the first line that
        // isn't one of Niwa's entries.
        if inside && (content.starts_with(b"0.0.0.0 ") || content.starts_with(b":: ")) {
            continue;
        }
        inside = false;
        out.extend_from_slice(line);
    }
    out
}

/// Takes Niwa's block out of the hosts file, if it's there. Returns whether
/// the file had to change.
pub fn clear() -> io::Result<bool> {
    let path = path();
    let text = match fs::read(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    let wanted = without_block(&text);
    if wanted == text {
        return Ok(false);
    }
    replace(&path, &wanted)?;
    if test_path().is_none() {
        flush_dns();
    }
    Ok(true)
}

/// Writes the new file beside the old one and swaps it in with
/// `ReplaceFileW`, which keeps the old file's permissions and attributes. A
/// crash midway leaves the old file whole, never half of one.
fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if !path.exists() {
        return fs::write(path, bytes);
    }
    let read_only = fs::metadata(path)?.permissions().readonly();
    if read_only {
        set_read_only(path, false)?;
    }
    let mut fresh = path.as_os_str().to_owned();
    fresh.push(".niwa-new");
    let fresh = PathBuf::from(fresh);
    let written = fs::File::create(&fresh).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    // SAFETY: plain FFI call with two valid paths and no backup file.
    let swapped = written.and_then(|()| {
        unsafe { ReplaceFileW(&HSTRING::from(path), &HSTRING::from(fresh.as_path()), None, REPLACE_FILE_FLAGS(0), None, None) }
            .map_err(io::Error::other)
    });
    if swapped.is_err() {
        let _ = fs::remove_file(&fresh);
    }
    if read_only {
        let _ = set_read_only(path, true);
    }
    swapped
}

fn set_read_only(path: &Path, on: bool) -> io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_readonly(on);
    fs::set_permissions(path, permissions)
}

/// Makes Windows forget the addresses it has already looked up, so a site
/// the old block covered loads again now rather than when the cache expires.
fn flush_dns() {
    let _ = Command::new("ipconfig")
        .arg("/flushdns")
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOWS_DEFAULT: &str = "# Copyright (c) 1993-2009 Microsoft Corp.\r\n#\r\n#\t127.0.0.1       localhost\r\n#\t::1             localhost\r\n";
    /// A block as earlier versions of Niwa wrote it.
    const OLD_BLOCK: &str = "# BEGIN Niwa: sites blocked during a focus session. Niwa removes this block when the session ends.\r\n0.0.0.0 reddit.com\r\n:: reddit.com\r\n# END Niwa\r\n";

    fn text(bytes: &[u8]) -> &str {
        std::str::from_utf8(bytes).unwrap()
    }

    #[test]
    fn takes_out_an_old_block() {
        let blocked = format!("{WINDOWS_DEFAULT}{OLD_BLOCK}");
        assert_eq!(text(&without_block(blocked.as_bytes())), WINDOWS_DEFAULT);
    }

    #[test]
    fn keeps_the_users_own_lines_around_a_block() {
        let edited = format!("10.0.0.2 nas\n{}10.0.0.3 printer\n", OLD_BLOCK.replace("\r\n", "\n"));
        assert_eq!(text(&without_block(edited.as_bytes())), "10.0.0.2 nas\n10.0.0.3 printer\n");
    }

    #[test]
    fn a_block_missing_its_end_line_is_still_removed() {
        let broken = "10.0.0.2 nas\r\n# BEGIN Niwa: sites\r\n0.0.0.0 x.com\r\n:: x.com\r\n10.0.0.3 printer\r\n";
        assert_eq!(text(&without_block(broken.as_bytes())), "10.0.0.2 nas\r\n10.0.0.3 printer\r\n");
    }

    #[test]
    fn a_file_without_a_block_is_left_as_it_is() {
        assert_eq!(without_block(WINDOWS_DEFAULT.as_bytes()), WINDOWS_DEFAULT.as_bytes());
        assert_eq!(without_block(b"10.0.0.2 nas"), b"10.0.0.2 nas");
    }

    #[test]
    fn clears_a_real_file() {
        let dir = std::env::temp_dir().join(format!("niwa-hosts-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("hosts");
        fs::write(&file, format!("{WINDOWS_DEFAULT}{OLD_BLOCK}")).unwrap();
        set_read_only(&file, true).unwrap();
        // `replace` is the same call `clear` makes, on a file of the test's own.
        let text = fs::read(&file).unwrap();
        replace(&file, &without_block(&text)).unwrap();
        assert_eq!(fs::read(&file).unwrap(), WINDOWS_DEFAULT.as_bytes());
        assert!(fs::metadata(&file).unwrap().permissions().readonly());
        assert!(!dir.join("hosts.niwa-new").exists());
        set_read_only(&file, false).unwrap();
        fs::remove_dir_all(&dir).unwrap();
    }
}
