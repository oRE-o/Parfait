//! In-process path identity and application-owned file protection, not a security boundary.
use std::path::{Path, PathBuf};
pub(crate) fn resolved(path: &Path) -> anyhow::Result<PathBuf> {
    match path.symlink_metadata() {
        Ok(_) => return Ok(path.canonicalize()?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Choose a project file name"))?;
    Ok(parent.canonicalize()?.join(name))
}
pub(crate) fn allowed(path: &Path, data: &Path) -> anyhow::Result<PathBuf> {
    let target = resolved(path)?;
    let data = data.canonicalize()?;
    anyhow::ensure!(
        !target.starts_with(data.join("recovery")),
        "Choose a file outside the recovery directory"
    );
    if target.exists()
        && let Ok(entries) = std::fs::read_dir(data.join("recovery"))
    {
        for (index, entry) in entries.enumerate() {
            anyhow::ensure!(
                index < 128,
                "Too many recovery directories to safely inspect this target"
            );
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.len() != 32
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                continue;
            }
            for suffix in ["", "-wal", "-shm", "-journal"] {
                anyhow::ensure!(
                    !same_source(
                        &target,
                        &entry.path().join(format!("recovery.parfait{suffix}"))
                    ),
                    "Choose a file outside the application's document recovery files"
                );
            }
        }
    }
    for name in [
        "recovery.parfait",
        "before-open.parfait",
        "before-new.parfait",
        "preferences.sqlite",
        "session.sqlite",
        "session.lock",
    ] {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            anyhow::ensure!(
                !same_source(&target, &data.join(format!("{name}{suffix}"))),
                "Choose a file outside the application's working files"
            );
        }
    }
    Ok(target)
}

/// Canonical paths are the portable policy; Unix file IDs also recognize hard links.
pub(crate) fn same_source(left: &Path, right: &Path) -> bool {
    if let (Ok(left), Ok(right)) = (resolved(left), resolved(right)) {
        if left == right {
            return true;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let (Ok(left), Ok(right)) = (left.metadata(), right.metadata()) {
                return left.dev() == right.dev() && left.ino() == right.ino();
            }
        }
    }
    false
}
