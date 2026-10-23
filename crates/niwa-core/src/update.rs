//! Checks a Niwa installer before the Strict service runs it.
//!
//! Updates normally ask for admin, because the installer replaces the
//! service. Instead the card hands a downloaded installer to the service,
//! which already has those rights (approved Oct 6, 2026). The service runs it
//! only when it is signed with Niwa's update key, the signature names the
//! version the card asked for, and that version is newer than the service:
//! so the most anyone can get the service to run is a genuine, newer Niwa.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use minisign_verify::{PublicKey, Signature};

/// The public half of Niwa's update key (`~/.tauri/niwa-updater.key`), as
/// base64 of the minisign key file. `plugins.updater.pubkey` in
/// `app/src-tauri/tauri.conf.json` is the same; a test keeps them so.
pub const PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDNGMzIwNTA2NUY1Rjg3RkEKUldUNmgxOWZCZ1V5UDRodkpvMklJbE5HT3haNXZrNU1UakpmOFRjSDJMdnlCcmlZTmtmMjBYaE0K";

/// The largest installer the service will read. Niwa's is a few megabytes.
pub const MAX_INSTALLER_BYTES: u64 = 256 * 1024 * 1024;

/// Checks that `installer` is signed with Niwa's update key, for `version`
/// exactly, and that `version` is newer than `current`.
pub fn check_installer(installer: &[u8], signature: &str, version: &str, current: &str) -> Result<(), String> {
    check_with(PUBLIC_KEY, installer, signature, version, current)
}

fn check_with(key: &str, installer: &[u8], signature: &str, version: &str, current: &str) -> Result<(), String> {
    const NOT_NIWA: &str = "That update isn't signed by Niwa.";
    let decode = |text: &str| -> Option<String> { String::from_utf8(BASE64.decode(text.trim()).ok()?).ok() };
    let key = decode(key)
        .and_then(|key| PublicKey::decode(&key).ok())
        .ok_or("Niwa's update key couldn't be read.")?;
    let signature = decode(signature)
        .and_then(|signature| Signature::decode(&signature).ok())
        .ok_or(NOT_NIWA)?;
    key.verify(installer, &signature, false).map_err(|_| NOT_NIWA)?;
    // The trusted comment is covered by the signature, so only now can it
    // be believed. Without it an old release could be passed off as new.
    let signed = signature
        .trusted_comment()
        .split('\t')
        .find_map(|field| field.strip_prefix("version:"));
    if signed != Some(version) {
        return Err("That update was signed for a different version.".into());
    }
    if !is_newer(version, current) {
        return Err(format!("Niwa {current} is already as new as {version}."));
    }
    Ok(())
}

/// "1.2.3" style versions, compared part by part. Anything else (a
/// pre-release, say) is never newer.
pub fn is_newer(version: &str, than: &str) -> bool {
    fn parts(version: &str) -> Option<[u64; 3]> {
        let mut parts = version.split('.').map(|part| part.parse::<u64>().ok());
        let parsed = [parts.next()??, parts.next()??, parts.next()??];
        parts.next().is_none().then_some(parsed)
    }
    matches!((parts(version), parts(than)), (Some(version), Some(than)) if version > than)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A throwaway key made for these tests, never used for a release.
    const TEST_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDQ3MDUyOEZEMkQ1NTlDQjAKUldTd25GVXQvU2dGUjU1bm56QVYwL3lYdU1lb0NpSUdJc0JnY2Z1YzRUMEVjTWNsK2JQNHA3Wk8K";
    const INSTALLER: &[u8] = b"not really an installer";
    /// `INSTALLER` signed with the test key for version 0.9.0.
    const SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVTd25GVXQvU2dGUjJ0OXlhNTdKMGYrZmxOdW1KRUF5SEF3WCttcWQxV1hvN3NDazB5SkE1dWJkbHZzYXlkOEZubHZHbnlvWHlJSkRLMGs3QUNZT3UwODlRK2V4OHh1end3PQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzkxMzM0MDczCWZpbGU6aW5zdGFsbGVyLmJpbgl2ZXJzaW9uOjAuOS4wCnk5Q1F4MTcvaVNwSGt6TVNkK3JTS0tHV0VEaE9nMzBuMnBkaE5EdlFDaW9pUGNINlk5YkNPV3FQcERFSyt2TGdDN2NOM3J4R2dUdzZLRm5jMW1QNkRBPT0K";
    /// "not really an installer either", signed with the test key and no version.
    const UNVERSIONED: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVTd25GVXQvU2dGUit0WklDN2o4MFlYWXhTazBmSE5EcWxpOWlBNy82RWpKT1lFS0ttNElCT1JKZFVMU1JudWhoakhieE4wdjJDdXpIdUUyQytOUXZFZnNEWk1tb3M0blF3PQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzkxMzM0MDczCWZpbGU6dW52ZXJzaW9uZWQuYmluCjU5OWVmRDNRbkl6bXIrbUFGcm93N3hHOEJhalhPb1R2ZERiL1Ird1IwVVc0dkdqcnhRUStsUWJoZURjK1pKelBvZDlVdERTN0xoM25meDY3THZKUUFnPT0K";

    #[test]
    fn a_signed_newer_installer_passes() {
        assert_eq!(check_with(TEST_KEY, INSTALLER, SIGNATURE, "0.9.0", "0.2.0"), Ok(()));
    }

    #[test]
    fn a_changed_installer_fails() {
        let mut changed = INSTALLER.to_vec();
        changed[0] ^= 1;
        assert!(check_with(TEST_KEY, &changed, SIGNATURE, "0.9.0", "0.2.0").is_err());
    }

    #[test]
    fn another_key_fails() {
        // Niwa's real key didn't make the test signature.
        assert!(check_with(PUBLIC_KEY, INSTALLER, SIGNATURE, "0.9.0", "0.2.0").is_err());
        assert!(check_installer(INSTALLER, SIGNATURE, "0.9.0", "0.2.0").is_err());
    }

    #[test]
    fn the_signed_version_must_match_and_be_newer() {
        assert!(check_with(TEST_KEY, INSTALLER, SIGNATURE, "9.9.9", "0.2.0").is_err());
        assert!(check_with(TEST_KEY, INSTALLER, SIGNATURE, "0.9.0", "0.9.0").is_err());
        assert!(check_with(TEST_KEY, INSTALLER, SIGNATURE, "0.9.0", "1.0.0").is_err());
        assert!(check_with(TEST_KEY, b"not really an installer either", UNVERSIONED, "0.9.0", "0.2.0").is_err());
    }

    #[test]
    fn rubbish_signatures_fail() {
        assert!(check_with(TEST_KEY, INSTALLER, "", "0.9.0", "0.2.0").is_err());
        assert!(check_with(TEST_KEY, INSTALLER, "not base64!", "0.9.0", "0.2.0").is_err());
    }

    #[test]
    fn versions_compare_part_by_part() {
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("0.3.0-beta", "0.2.0"));
        assert!(!is_newer("0.3", "0.2.0"));
        assert!(!is_newer("0.3.0.1", "0.2.0"));
    }

    #[test]
    fn the_key_and_version_match_the_apps() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../../../app/src-tauri/tauri.conf.json")).unwrap();
        assert_eq!(conf["plugins"]["updater"]["pubkey"], PUBLIC_KEY);
        // The service compares against its own version, so the app's must be the same.
        assert_eq!(conf["version"], env!("CARGO_PKG_VERSION"));
    }
}
