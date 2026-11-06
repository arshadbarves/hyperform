//! bubblewrap arguments: a read-only view of `/`, fresh `/dev` and
//! `/proc`, read-write binds for the writable paths and the shared temp
//! directories, and an optional private network namespace.

use std::path::{Path, PathBuf};

/// Bound read-write when present, so temp files (including the working
/// directory probe) are shared with the host rather than discarded.
const SHARED_TEMP: &[&str] = &["/tmp", "/var/tmp"];

/// Everything before `--` and the shell's own arguments. Existing
/// `read_only` paths are bound read-only over the writable binds; a mount
/// cannot cover a path that does not exist yet.
pub(crate) fn args(
    writable: &[PathBuf],
    read_only: &[PathBuf],
    allow_network: bool,
) -> Vec<String> {
    let mut args: Vec<String> = ["--ro-bind", "/", "/", "--dev", "/dev", "--proc", "/proc"]
        .map(str::to_string)
        .to_vec();
    let temp = SHARED_TEMP
        .iter()
        .map(PathBuf::from)
        .filter(|dir| dir.is_dir() && !writable.iter().any(|w| dir.starts_with(w)));
    for path in temp.chain(writable.iter().cloned()) {
        bind(&mut args, "--bind", &path);
    }
    for path in read_only.iter().filter(|path| path.exists()) {
        bind(&mut args, "--ro-bind", path);
    }
    if !allow_network {
        args.push("--unshare-net".to_string());
    }
    args.push("--die-with-parent".to_string());
    args.push("--".to_string());
    args
}

fn bind(args: &mut Vec<String>, flag: &str, path: &Path) {
    let path = path.to_string_lossy().into_owned();
    args.push(flag.to_string());
    args.push(path.clone());
    args.push(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_writable_paths_after_the_read_only_root() {
        let existing = tempfile::tempdir().unwrap();
        let protected = existing.path().to_string_lossy().into_owned();
        let read_only = [
            existing.path().to_path_buf(),
            PathBuf::from("/nope/.mcp.json"),
        ];
        let args = args(&[PathBuf::from("/work/proj")], &read_only, false);
        assert_eq!(&args[..3], ["--ro-bind", "/", "/"]);
        let bind = args.iter().position(|a| a == "/work/proj").unwrap();
        assert_eq!(args[bind - 1], "--bind");
        assert_eq!(args[bind + 1], "/work/proj");
        let ro = args.iter().position(|a| *a == protected).unwrap();
        assert!(ro > bind && args[ro - 1] == "--ro-bind");
        assert!(!args.iter().any(|a| a.contains("/nope")));
        assert!(args.contains(&"--unshare-net".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("--"));
        assert_eq!(args[args.len() - 2], "--die-with-parent");
    }

    #[test]
    fn network_stays_shared_when_allowed() {
        let args = args(&[], &[], true);
        assert!(!args.contains(&"--unshare-net".to_string()));
    }
}
