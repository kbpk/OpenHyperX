//! Offline crash-recovery snapshots for file drafts. A snapshot is never a
//! device backup, an apply plan, or permission to write onboard memory.

use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use hyperx_core::SoftwareProfile;
use serde::{Deserialize, Serialize};

use crate::{parse_profile, ProfileDocument, MAX_PROFILE_BYTES};

const VERSION: u32 = 1;
const MAX_RECOVERY_BYTES: usize = 3 * MAX_PROFILE_BYTES + 16 * 1024;
static NEXT_SESSION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryClient {
    Tui,
    Gui,
}

impl RecoveryClient {
    const fn id(self) -> &'static str {
        match self {
            Self::Tui => "tui",
            Self::Gui => "gui",
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryRecord {
    version: u32,
    current: SoftwareProfile,
    baseline: SoftwareProfile,
    /// Original opened/saved FILE bytes are UTF-8 (as required by ProfileDocument).
    /// Keeping them intact preserves the exact-byte stale-file gate after restart.
    baseline_text: Option<String>,
    source_path: Option<PathBuf>,
}

/// Each running client owns only its new snapshot. A completed snapshot gets a
/// fresh filename before the old one is removed, so a failed write cannot
/// destroy the last recoverable draft. Other processes and clients never share
/// a mutable snapshot pathname.
pub struct DraftRecoveryStore {
    directory: PathBuf,
    client: RecoveryClient,
    session: String,
    sequence: u64,
    active: Option<PathBuf>,
}

impl DraftRecoveryStore {
    /// Use a private application-state directory, not the profile's folder.
    /// Callers may pass a separate explicit directory in tests or portable apps.
    pub fn default_directory() -> Result<PathBuf> {
        #[cfg(windows)]
        let base = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .context("LOCALAPPDATA is unavailable; supply a recovery directory")?;
        #[cfg(target_os = "macos")]
        let base = env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME is unavailable; supply a recovery directory")?
            .join("Library/Application Support");
        #[cfg(all(unix, not(target_os = "macos")))]
        let base = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
            .context("XDG_STATE_HOME and HOME are unavailable; supply a recovery directory")?;
        Ok(base.join("OpenHyperX").join("draft-recovery"))
    }

    pub fn new(directory: &Path, client: RecoveryClient) -> Result<Self> {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
            builder.mode(0o700);
            builder.create(directory).with_context(|| {
                format!("cannot create recovery directory {}", directory.display())
            })?;
            let metadata = fs::symlink_metadata(directory)?;
            if !metadata.file_type().is_dir() || metadata.permissions().mode() & 0o077 != 0 {
                bail!(
                    "recovery directory {} must be a private, non-symlinked directory (mode 0700)",
                    directory.display()
                );
            }
        }
        #[cfg(not(unix))]
        {
            builder.create(directory).with_context(|| {
                format!("cannot create recovery directory {}", directory.display())
            })?;
            if !fs::symlink_metadata(directory)?.file_type().is_dir() {
                bail!("recovery path {} is not a directory", directory.display());
            }
        }
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let nonce = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
        Ok(Self {
            directory: directory.to_owned(),
            client,
            session: format!("{stamp:032x}-{:08x}-{nonce:016x}", process::id()),
            sequence: 0,
            active: None,
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Snapshot only an unsaved FILE draft. A clean document clears this
    /// process's last snapshot; it never deletes another process's files.
    pub fn capture(&mut self, document: &ProfileDocument) -> Result<Option<PathBuf>> {
        if !document.dirty() {
            self.clear_owned()?;
            return Ok(None);
        }
        let source_path = document
            .path
            .as_ref()
            .map(|path| -> Result<PathBuf> {
                if path.is_absolute() {
                    Ok(path.clone())
                } else {
                    Ok(env::current_dir()?.join(path))
                }
            })
            .transpose()?;
        let baseline_text = document
            .baseline_file
            .as_ref()
            .map(|bytes| String::from_utf8(bytes.clone()))
            .transpose()
            .context("opened FILE baseline is not UTF-8")?;
        let record = RecoveryRecord {
            version: VERSION,
            current: document.profile.clone(),
            baseline: document.baseline.clone(),
            baseline_text,
            source_path,
        };
        let encoded = toml::to_string(&record)?.into_bytes();
        if encoded.len() > MAX_RECOVERY_BYTES {
            bail!("recovery snapshot exceeds its bounded size");
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .context("recovery sequence exhausted")?;
        let name = format!(
            "draft-{}-{}-{:016x}.toml",
            self.client.id(),
            self.session,
            self.sequence
        );
        let destination = self.directory.join(name);
        let temporary = destination.with_extension("tmp");
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .with_context(|| format!("cannot create recovery snapshot {}", temporary.display()))?;
        file.write_all(&encoded)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &destination).with_context(|| {
            format!(
                "cannot finalize recovery snapshot {}",
                destination.display()
            )
        })?;
        let previous = self.active.replace(destination.clone());
        if let Some(previous) = previous {
            fs::remove_file(&previous).with_context(|| {
                format!(
                    "new recovery snapshot is safe at {}, but old snapshot {} could not be removed",
                    destination.display(),
                    previous.display()
                )
            })?;
        }
        Ok(Some(destination))
    }

    pub fn clear_owned(&mut self) -> Result<()> {
        if let Some(path) = self.active.take() {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    self.active = Some(path.clone());
                    return Err(error).with_context(|| {
                        format!("cannot clear recovery snapshot {}", path.display())
                    });
                }
            }
        }
        Ok(())
    }

    /// Enumerate only complete snapshot names for one client. Malformed files
    /// remain visible to callers and are not silently deleted.
    pub fn list(&self) -> Result<Vec<PathBuf>> {
        let prefix = format!("draft-{}-", self.client.id());
        let mut paths = Vec::new();
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&prefix) && name.ends_with(".toml") {
                paths.push(entry.path());
            }
        }
        paths.sort();
        Ok(paths)
    }

    pub fn load(&self, path: &Path) -> Result<ProfileDocument> {
        self.check_candidate(path)?;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_RECOVERY_BYTES as u64 {
            bail!("recovery snapshot must be a bounded regular file");
        }
        let mut encoded = String::new();
        File::open(path)?
            .take((MAX_RECOVERY_BYTES + 1) as u64)
            .read_to_string(&mut encoded)?;
        if encoded.len() > MAX_RECOVERY_BYTES {
            bail!("recovery snapshot exceeds its bounded size");
        }
        let record: RecoveryRecord = toml::from_str(&encoded)
            .with_context(|| format!("cannot parse recovery snapshot {}", path.display()))?;
        if record.version != VERSION || record.current == record.baseline {
            bail!("recovery snapshot has an unsupported version or no unsaved changes");
        }
        if record.source_path.is_some() != record.baseline_text.is_some() {
            bail!("recovery snapshot has an incomplete FILE baseline");
        }
        if let Some(text) = &record.baseline_text {
            if parse_profile(text)? != record.baseline {
                bail!("recovery snapshot FILE bytes do not match its baseline profile");
            }
        }
        // A restored document keeps the original exact-byte stale-file gate.
        // No read or write of that original file occurs until an explicit user
        // action, and an on-disk change still blocks overwrite.
        Ok(ProfileDocument {
            profile: record.current,
            baseline: record.baseline,
            baseline_file: record.baseline_text.map(String::into_bytes),
            path: record.source_path,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    /// Take responsibility for one explicitly selected previous snapshot.
    /// Loading alone is read-only; only adoption lets a later successful
    /// capture or clean save retire that particular old snapshot.
    pub fn adopt(&mut self, path: &Path) -> Result<ProfileDocument> {
        if self.active.is_some() {
            bail!("this recovery session already owns a snapshot");
        }
        let document = self.load(path)?;
        self.active = Some(path.to_owned());
        Ok(document)
    }

    /// Remove only a candidate in this client's recovery directory, after an
    /// explicit UI decision to discard it or after a restored draft was saved.
    pub fn discard(&self, path: &Path) -> Result<()> {
        self.check_candidate(path)?;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() {
            bail!("refusing to discard a non-regular recovery snapshot");
        }
        fs::remove_file(path)
            .with_context(|| format!("cannot discard recovery snapshot {}", path.display()))
    }

    fn check_candidate(&self, path: &Path) -> Result<()> {
        let name = path.file_name().and_then(|name| name.to_str());
        let prefix = format!("draft-{}-", self.client.id());
        if path.parent() != Some(self.directory.as_path())
            || !name.is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".toml"))
        {
            bail!("path is not a {} recovery snapshot", self.client.id());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::{DraftRecoveryStore, RecoveryClient};
    use crate::{parse_profile, ProfileDocument};

    struct Files(PathBuf);

    impl Files {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "openhyperx-recovery-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }

        fn store(&self, client: RecoveryClient) -> DraftRecoveryStore {
            DraftRecoveryStore::new(&self.path("private"), client).unwrap()
        }
    }

    impl Drop for Files {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn changed_document() -> ProfileDocument {
        let profile = parse_profile(include_str!(
            "../../../examples/profiles/pulsefire-raid.toml"
        ))
        .unwrap();
        let mut document = ProfileDocument::from_profile(profile);
        let mut edited = document.profile().clone();
        edited.name = "Recovered draft".into();
        document.replace(edited);
        document
    }

    #[test]
    fn recovery_keeps_original_file_baseline_and_stale_overwrite_gate() {
        let files = Files::new();
        let source = files.path("source.toml");
        let text = include_str!("../../../examples/profiles/pulsefire-raid.toml");
        fs::write(&source, format!("# keep formatting\n{text}")).unwrap();
        let mut document = ProfileDocument::open(&source).unwrap();
        let mut edited = document.profile().clone();
        edited.name = "Recovered name".into();
        document.replace(edited.clone());
        let mut store = files.store(RecoveryClient::Tui);
        let snapshot = store.capture(&document).unwrap().unwrap();
        assert!(snapshot.exists());
        drop(store);

        let store = files.store(RecoveryClient::Tui);
        assert_eq!(store.list().unwrap(), vec![snapshot.clone()]);
        let restored = store.load(&snapshot).unwrap();
        assert!(restored.dirty());
        assert_eq!(restored.path(), Some(source.as_path()));
        assert_eq!(restored.profile(), &edited);
        assert!(restored
            .diff()
            .unwrap()
            .metadata
            .iter()
            .any(|change| change.field == "name"));

        let changed_on_disk = format!("# another process edited this file\n{text}");
        fs::write(&source, &changed_on_disk).unwrap();
        let mut restored = store.load(&snapshot).unwrap();
        assert!(restored.save_overwrite_with_backup().is_err());
        assert_eq!(fs::read_to_string(&source).unwrap(), changed_on_disk);
        assert!(snapshot.exists());
    }

    #[test]
    fn sessions_and_clients_never_replace_each_others_snapshots() {
        let files = Files::new();
        let mut tui_a = files.store(RecoveryClient::Tui);
        let mut tui_b = files.store(RecoveryClient::Tui);
        let mut gui = files.store(RecoveryClient::Gui);
        let mut document = changed_document();
        let first = tui_a.capture(&document).unwrap().unwrap();
        let other = tui_b.capture(&document).unwrap().unwrap();
        let gui_path = gui.capture(&document).unwrap().unwrap();
        let mut next = document.profile().clone();
        next.name = "Second draft".into();
        document.replace(next);
        let latest = tui_a.capture(&document).unwrap().unwrap();
        assert!(!first.exists());
        assert!(other.exists() && gui_path.exists() && latest.exists());
        assert_eq!(tui_a.list().unwrap().len(), 2);
        assert_eq!(gui.list().unwrap(), vec![gui_path]);
        tui_a.clear_owned().unwrap();
        assert!(!latest.exists() && other.exists());
        document.undo();
        document.undo();
        assert!(!document.dirty());
        assert!(tui_b.capture(&document).unwrap().is_none());
        assert!(!other.exists());
    }

    #[test]
    fn malformed_and_out_of_scope_candidates_are_rejected_without_deletion() {
        let files = Files::new();
        let store = files.store(RecoveryClient::Tui);
        let bad = store.directory().join("draft-tui-broken.toml");
        fs::write(&bad, "broken = [").unwrap();
        assert_eq!(store.list().unwrap(), vec![bad.clone()]);
        assert!(store.load(&bad).is_err());
        assert!(bad.exists());
        assert!(store.load(Path::new("../other.toml")).is_err());
        assert!(store.discard(&files.path("unrelated.toml")).is_err());
        assert!(bad.exists());
        store.discard(&bad).unwrap();
        assert!(!bad.exists());
    }

    #[test]
    fn clean_or_saved_documents_leave_no_active_snapshot() {
        let files = Files::new();
        let mut store = files.store(RecoveryClient::Tui);
        let mut document = changed_document();
        assert!(store.capture(&document).unwrap().is_some());
        document.save_as(&files.path("new.toml")).unwrap();
        assert!(!document.dirty());
        assert!(store.capture(&document).unwrap().is_none());
        assert!(store.list().unwrap().is_empty());
    }
}
