//! Reads a program's version resource (the Details tab in Explorer).

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};

use crate::blocklist::FileIdentity;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The product name and original file name an executable declares, or `None`
/// if it has no version resource.
pub fn file_identity(path: &Path) -> Option<FileIdentity> {
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let path = PCWSTR(path.as_ptr());

    // SAFETY: `path` is NUL-terminated and outlives the calls; `data` is as
    // large as the API said it needs.
    let data = unsafe {
        let size = GetFileVersionInfoSizeW(path, None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(path, None, size, data.as_mut_ptr() as *mut c_void).ok()?;
        data
    };

    // The resource holds one string table per language; take the first.
    let (language, code_page) = {
        let (pointer, len) = query(&data, r"\VarFileInfo\Translation")?;
        if len < 4 {
            return None;
        }
        // SAFETY: the API returned at least four bytes at `pointer`, inside `data`.
        let words = unsafe { std::slice::from_raw_parts(pointer as *const u16, 2) };
        (words[0], words[1])
    };
    let string = |name: &str| {
        let key = format!(r"\StringFileInfo\{language:04x}{code_page:04x}\{name}");
        let (pointer, len) = query(&data, &key)?;
        // SAFETY: for string values `len` counts UTF-16 units at `pointer`,
        // inside `data`.
        let units = unsafe { std::slice::from_raw_parts(pointer as *const u16, len as usize) };
        // Some programs pad the value with a NUL and stray bytes after it.
        let text = String::from_utf16_lossy(units);
        Some(text.split('\0').next().unwrap_or_default().trim().to_owned())
    };

    Some(FileIdentity {
        product_name: string("ProductName").unwrap_or_default(),
        original_filename: string("OriginalFilename").unwrap_or_default(),
    })
}

fn query(data: &[u8], key: &str) -> Option<(*const c_void, u32)> {
    let key = wide(key);
    let mut pointer: *mut c_void = std::ptr::null_mut();
    let mut len = 0u32;
    // SAFETY: `data` is a version block from `GetFileVersionInfoW`; the out
    // pointers are valid. The returned pointer points into `data`.
    let found = unsafe {
        VerQueryValueW(
            data.as_ptr() as *const c_void,
            PCWSTR(key.as_ptr()),
            &mut pointer,
            &mut len,
        )
    };
    (found.as_bool() && !pointer.is_null() && len > 0).then_some((pointer as *const c_void, len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_windows_program() {
        let system = std::env::var_os("SystemRoot").unwrap();
        let identity = file_identity(&Path::new(&system).join("System32").join("cmd.exe")).unwrap();
        assert!(identity.product_name.contains("Windows"), "{identity:?}");
        // Windows reports its own programs through their language file: "Cmd.Exe.MUI".
        assert!(
            identity.original_filename.to_lowercase().starts_with("cmd.exe"),
            "{identity:?}"
        );
    }

    #[test]
    fn files_without_a_version_resource_have_no_identity() {
        let file = std::env::temp_dir().join(format!("niwa-identity-{}.txt", std::process::id()));
        std::fs::write(&file, "plain text").unwrap();
        let identity = file_identity(&file);
        std::fs::remove_file(&file).unwrap();
        assert_eq!(identity, None);
        assert_eq!(file_identity(Path::new(r"C:\does\not\exist.exe")), None);
    }
}
