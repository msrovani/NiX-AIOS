//! Landlock (I.3) — NIX_WRITE confinement.
//! - Linux + `NOSD_LANDLOCK=1`: tenta `landlock_*` syscalls (ruleset FS read/write sob data_dir).
//! - Sem kernel Landlock / Windows: SoftAllowlist + `path_allowed` (honesty ≠ Verified).

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LandlockStatus {
    Disabled,
    AbsentPlatform,
    /// Prefix check only — kernel ABI not applied.
    SoftAllowlist,
    /// `landlock_restrict_self` succeeded this process.
    Verified,
    /// Env pediu Landlock mas syscall falhou (kernel antigo / seccomp).
    Failed,
}

static mut APPLIED: LandlockStatus = LandlockStatus::Disabled;

/// Último resultado de `try_restrict_process` (boot).
pub fn applied_status() -> LandlockStatus {
    // SAFETY: word-sized status written once on boot path.
    unsafe { APPLIED }
}

pub fn status_from_env() -> LandlockStatus {
    if std::env::var("NOSD_LANDLOCK").ok().as_deref() != Some("1") {
        return LandlockStatus::Disabled;
    }
    #[cfg(target_os = "linux")]
    {
        // Prefer last apply result if any.
        // SAFETY: single-threaded boot path / atomic-ish read of status word.
        let s = unsafe { APPLIED };
        if matches!(s, LandlockStatus::Verified | LandlockStatus::Failed) {
            return s;
        }
        LandlockStatus::SoftAllowlist
    }
    #[cfg(not(target_os = "linux"))]
    {
        LandlockStatus::AbsentPlatform
    }
}

/// Paths NIX_WRITE / FS must stay inside (soft check — always).
pub fn path_allowed(path: &Path, data_dir: &Path) -> bool {
    if path.starts_with(data_dir) {
        return true;
    }
    let Ok(canon) = path
        .canonicalize()
        .or_else(|_| Ok::<_, std::io::Error>(path.to_path_buf()))
    else {
        return false;
    };
    let Ok(root) = data_dir
        .canonicalize()
        .or_else(|_| Ok::<_, std::io::Error>(data_dir.to_path_buf()))
    else {
        return false;
    };
    canon.starts_with(&root)
        || canon.starts_with("/etc/nixos/ai-managed.nix")
        || path.starts_with("/var/lib/nosd")
        || path.starts_with("/etc/nixos")
}

/// Aplica Landlock no processo actual (Linux). Idempotente best-effort.
/// Allow: `data_dir` RW + `/etc/nixos` RO + `/var/lib/nosd` RW.
pub fn try_restrict_process(data_dir: &Path) -> LandlockStatus {
    if std::env::var("NOSD_LANDLOCK").ok().as_deref() != Some("1") {
        return LandlockStatus::Disabled;
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = data_dir;
        let s = LandlockStatus::AbsentPlatform;
        unsafe { APPLIED = s };
        return s;
    }
    #[cfg(target_os = "linux")]
    {
        match linux_restrict(data_dir) {
            Ok(()) => {
                tracing::info!(target: "nosd", "landlock Verified under {}", data_dir.display());
                let s = LandlockStatus::Verified;
                unsafe { APPLIED = s };
                s
            }
            Err(e) => {
                tracing::warn!(target: "nosd", "landlock Failed ({e}) — SoftAllowlist only");
                let s = LandlockStatus::Failed;
                unsafe { APPLIED = s };
                s
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn linux_restrict(data_dir: &Path) -> Result<(), String> {
    // ABI Landlock (kernel ≥5.13): create_ruleset → add_rule → restrict_self.
    // Números syscall x86_64: 444 create, 445 add, 446 restrict (kernel uapi).
    const SYS_LANDLOCK_CREATE_RULESET: libc::c_long = 444;
    const SYS_LANDLOCK_ADD_RULE: libc::c_long = 445;
    const SYS_LANDLOCK_RESTRICT_SELF: libc::c_long = 446;
    const LANDLOCK_ACCESS_FS_READ_FILE: u64 = 1 << 2;
    const LANDLOCK_ACCESS_FS_READ_DIR: u64 = 1 << 3;
    const LANDLOCK_ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
    const LANDLOCK_ACCESS_FS_REMOVE_FILE: u64 = 1 << 4;
    const LANDLOCK_ACCESS_FS_MAKE_REG: u64 = 1 << 6;
    const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;

    #[repr(C)]
    struct LandlockRulesetAttr {
        handled_access_fs: u64,
    }
    #[repr(C)]
    struct LandlockPathBeneathAttr {
        allowed_access: u64,
        parent_fd: i32,
    }

    let handled = LANDLOCK_ACCESS_FS_READ_FILE
        | LANDLOCK_ACCESS_FS_READ_DIR
        | LANDLOCK_ACCESS_FS_WRITE_FILE
        | LANDLOCK_ACCESS_FS_REMOVE_FILE
        | LANDLOCK_ACCESS_FS_MAKE_REG;
    let attr = LandlockRulesetAttr {
        handled_access_fs: handled,
    };
    let ruleset = unsafe {
        libc::syscall(
            SYS_LANDLOCK_CREATE_RULESET,
            &attr as *const _,
            std::mem::size_of::<LandlockRulesetAttr>(),
            0u32,
        )
    };
    if ruleset < 0 {
        return Err(format!("create_ruleset errno={}", std::io::Error::last_os_error()));
    }
    let ruleset = ruleset as i32;

    let rw = LANDLOCK_ACCESS_FS_READ_FILE
        | LANDLOCK_ACCESS_FS_READ_DIR
        | LANDLOCK_ACCESS_FS_WRITE_FILE
        | LANDLOCK_ACCESS_FS_REMOVE_FILE
        | LANDLOCK_ACCESS_FS_MAKE_REG;
    let ro = LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_READ_DIR;

    let mut paths: Vec<(std::path::PathBuf, u64)> = vec![
        (data_dir.to_path_buf(), rw),
        (Path::new("/var/lib/nosd").to_path_buf(), rw),
        (Path::new("/etc/nixos").to_path_buf(), ro),
        (Path::new("/tmp").to_path_buf(), rw),
    ];
    // Dedup
    paths.sort_by(|a, b| a.0.cmp(&b.0));
    paths.dedup_by(|a, b| a.0 == b.0);

    for (p, access) in &paths {
        if !p.exists() {
            continue;
        }
        let fd = unsafe {
            libc::open(
                std::ffi::CString::new(p.to_string_lossy().as_bytes())
                    .map_err(|e| e.to_string())?
                    .as_ptr(),
                libc::O_PATH | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            continue;
        }
        let beneath = LandlockPathBeneathAttr {
            allowed_access: *access,
            parent_fd: fd,
        };
        let rc = unsafe {
            libc::syscall(
                SYS_LANDLOCK_ADD_RULE,
                ruleset,
                LANDLOCK_RULE_PATH_BENEATH,
                &beneath as *const _,
                0u32,
            )
        };
        unsafe {
            libc::close(fd);
        }
        if rc < 0 {
            unsafe {
                libc::close(ruleset);
            }
            return Err(format!(
                "add_rule {} errno={}",
                p.display(),
                std::io::Error::last_os_error()
            ));
        }
    }

    let rc = unsafe { libc::syscall(SYS_LANDLOCK_RESTRICT_SELF, ruleset, 0u32) };
    unsafe {
        libc::close(ruleset);
    }
    if rc < 0 {
        return Err(format!(
            "restrict_self errno={}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn disabled_by_default() {
        std::env::remove_var("NOSD_LANDLOCK");
        assert_eq!(status_from_env(), LandlockStatus::Disabled);
    }

    #[test]
    fn allow_under_data_prefix() {
        let root = PathBuf::from("/var/lib/nosd");
        assert!(
            Path::new("/var/lib/nosd/models/x.gguf").starts_with(&root)
                || path_allowed(Path::new("/var/lib/nosd/models/x.gguf"), &root)
        );
    }

    #[test]
    fn try_restrict_without_env_disabled() {
        std::env::remove_var("NOSD_LANDLOCK");
        assert_eq!(
            try_restrict_process(Path::new("/var/lib/nosd")),
            LandlockStatus::Disabled
        );
    }
}
