use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(unix)]
#[test]
fn signing_helpers_do_not_spawn_openssl_or_date() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new("qddns-provider-signing");
    let bin_dir = temp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let marker = temp.path().join("spawned");
    let script = format!(
        "#!/bin/sh\nprintf 'spawned %s\\n' \"$0 $*\" >> '{}'\nexit 42\n",
        marker.display()
    );
    for name in ["openssl", "date"] {
        let path = bin_dir.join(name);
        fs::write(&path, &script).unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }

    let old_path = std::env::var_os("PATH");
    std::env::set_var("PATH", &bin_dir);
    let digest = openssl_digest("sha256", "abc");
    let hmac = openssl_hmac_hex("sha256", b"key", b"data");
    let base64 = openssl_hmac_base64("sha1", b"key", b"data");
    let stamp = iso_timestamp(0);
    match old_path {
        Some(path) => std::env::set_var("PATH", path),
        None => std::env::remove_var("PATH"),
    }

    assert_eq!(
        digest.unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        hmac.unwrap(),
        "5031fe3d989c6d1537a013fa6e739da23463fdaec3b70137d828e36ace221bd0"
    );
    assert_eq!(base64.unwrap(), "EEFSxb/coHvGM+69RhmfAlXJ9J0=");
    assert_eq!(stamp, "1970-01-01T00:00:00Z");
    assert!(
        !marker.exists(),
        "signing helpers spawned external command: {}",
        fs::read_to_string(marker).unwrap_or_default()
    );
}
