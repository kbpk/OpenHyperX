//! Recoverable replacement of an already-opened FILE profile. This code never
//! inspects or writes device state; the old file remains in a numbered sibling
//! backup directory even after a successful save.

use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};

use crate::{encoded_profile_with_comments, read_profile_bytes, ProfileDocument};

fn reserve_backup_directory(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .context("profile path has no parent directory")?;
    let name = path.file_name().context("profile path has no filename")?;
    for number in 1..=9999 {
        let mut candidate: OsString = name.to_owned();
        candidate.push(format!(".openhyperx-backup-{number:04}"));
        let directory = parent.join(candidate);
        match fs::create_dir(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("cannot create recovery directory {}", directory.display())
                });
            }
        }
    }
    bail!(
        "no free numbered recovery directory beside {}",
        path.display()
    )
}

/// Create a destination only if absent. Hard-linking keeps a complete source
/// snapshot and avoids a partially written destination on normal filesystems;
/// create_new + copy is a conservative fallback when hard links are unavailable.
fn install_without_overwrite(source: &Path, destination: &Path) -> Result<()> {
    if fs::hard_link(source, destination).is_ok() {
        return Ok(());
    }
    let mut from = File::open(source)
        .with_context(|| format!("cannot reopen recovery snapshot {}", source.display()))?;
    let mut to = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .with_context(|| {
            format!(
                "cannot create {}; another file may have appeared; the recovery snapshot remains at {}",
                destination.display(),
                source.display()
            )
        })?;
    io::copy(&mut from, &mut to).with_context(|| {
        format!(
            "copy to {} failed; a partial destination may remain; recover from {}",
            destination.display(),
            source.display()
        )
    })?;
    to.sync_all().with_context(|| {
        format!(
            "sync of {} failed; recover from {}",
            destination.display(),
            source.display()
        )
    })
}

impl ProfileDocument {
    /// Replace only this document's opened/saved FILE path, after an exact-byte
    /// stale-file check. Returns a permanent backup path containing the old
    /// bytes, including comments and formatting. Never a mouse/onboard save.
    ///
    /// The old path is moved into a numbered backup directory, then the fully
    /// written replacement is installed without overwriting a path that another
    /// process created. A crash or I/O failure may leave the original path
    /// absent or partial; the backup and complete replacement remain available
    /// in the reported directory for manual recovery.
    pub fn save_overwrite_with_backup(&mut self) -> Result<PathBuf> {
        if !self.dirty() {
            bail!("no unsaved file edits to overwrite");
        }
        let path = self
            .path
            .as_ref()
            .context("no opened/saved file path; use Save NEW first")?;
        let baseline = self
            .baseline_file
            .as_ref()
            .context("no exact file baseline; use Save NEW first")?;
        let metadata = fs::symlink_metadata(path)
            .with_context(|| format!("cannot inspect existing profile {}", path.display()))?;
        if !metadata.file_type().is_file() || metadata.permissions().readonly() {
            bail!(
                "refusing to replace non-regular, symlinked or read-only profile {}",
                path.display()
            );
        }
        if read_profile_bytes(path)? != *baseline {
            bail!(
                "{} changed on disk since it was opened/saved; reopen or Save NEW instead of overwriting",
                path.display()
            );
        }
        let replacement = encoded_profile_with_comments(
            &self.profile,
            &["Offline application profile; not a device backup or an onboard save.".into()],
        )?;

        let directory = reserve_backup_directory(path)?;
        let old_path = directory.join(path.file_name().context("profile path has no filename")?);
        let new_path = directory.join("replacement.toml");
        let mut new_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&new_path)
            .with_context(|| {
                format!("cannot create replacement snapshot {}", new_path.display())
            })?;
        new_file.write_all(&replacement).with_context(|| {
            format!(
                "cannot write replacement snapshot {}; original profile remains untouched",
                new_path.display()
            )
        })?;
        new_file.sync_all().with_context(|| {
            format!(
                "cannot sync replacement snapshot {}; original profile remains untouched",
                new_path.display()
            )
        })?;
        drop(new_file);

        fs::rename(path, &old_path).with_context(|| {
            format!(
                "cannot move original {} to {}; original profile should remain untouched",
                path.display(),
                old_path.display()
            )
        })?;
        // A late external edit is retained in the moved file. Restore it only
        // with create-new semantics; never replace another process's new path.
        if !matches!(read_profile_bytes(&old_path), Ok(bytes) if bytes == *baseline) {
            let restored = install_without_overwrite(&old_path, path);
            match restored {
                Ok(()) => bail!(
                    "{} changed during save; original path restored without overwriting; recovery copy at {}",
                    path.display(),
                    old_path.display()
                ),
                Err(error) => bail!(
                    "{} changed during save; original could not be restored: {error:#}; recovery copy at {}",
                    path.display(),
                    old_path.display()
                ),
            }
        }
        install_without_overwrite(&new_path, path).with_context(|| {
            format!(
                "replacement was not installed; recover original from {} or new draft from {}",
                old_path.display(),
                new_path.display()
            )
        })?;
        if read_profile_bytes(path)? != replacement {
            bail!(
                "written profile {} no longer matches the draft; old file is preserved at {}",
                path.display(),
                old_path.display()
            );
        }
        self.baseline = self.profile.clone();
        self.baseline_file = Some(replacement);
        Ok(old_path)
    }
}
