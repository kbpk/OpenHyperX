//! Crash recovery for unaccepted TUI editor state. These records are not FILE
//! drafts, device backups, or permission to send reports.
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
use hyperx_app::{
    device_descriptor, DraftRecoveryStore, ProfileDocument, ProfileSection, RecoveryClient,
    MAX_PROFILE_BYTES,
};
use hyperx_core::SoftwareProfile;
use serde::{Deserialize, Serialize};

use crate::{
    app::{EditAction, Modal},
    editor::{Editor, EditorRecovery},
    macros::{MacroEditor, MacroRecovery},
};

const VERSION: u32 = 1;
const MAX_LOCAL_BYTES: usize = 3 * MAX_PROFILE_BYTES + 64 * 1024;
static NEXT_SESSION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum SavedSection {
    Performance,
    Buttons,
    Macros,
    Lighting,
    All,
}
impl From<ProfileSection> for SavedSection {
    fn from(value: ProfileSection) -> Self {
        match value {
            ProfileSection::Performance => Self::Performance,
            ProfileSection::Buttons => Self::Buttons,
            ProfileSection::Macros => Self::Macros,
            ProfileSection::Lighting => Self::Lighting,
            ProfileSection::All => Self::All,
        }
    }
}
impl From<SavedSection> for ProfileSection {
    fn from(value: SavedSection) -> Self {
        match value {
            SavedSection::Performance => Self::Performance,
            SavedSection::Buttons => Self::Buttons,
            SavedSection::Macros => Self::Macros,
            SavedSection::Lighting => Self::Lighting,
            SavedSection::All => Self::All,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum SavedAction {
    Section { section: SavedSection },
    Open,
    SaveAs,
    Resolve,
    Omit,
    ImportMacro,
    ProfileName,
    StageDpi { index: usize },
    StageColor { index: usize },
    AddStage,
    ZoneColor { zone: String },
}
impl From<EditAction> for SavedAction {
    fn from(value: EditAction) -> Self {
        match value {
            EditAction::Section(section) => Self::Section {
                section: section.into(),
            },
            EditAction::Open | EditAction::OpenRecovered => Self::Open,
            EditAction::SaveAs => Self::SaveAs,
            EditAction::Resolve => Self::Resolve,
            EditAction::Omit => Self::Omit,
            EditAction::ImportMacro => Self::ImportMacro,
            EditAction::ProfileName => Self::ProfileName,
            EditAction::StageDpi(index) => Self::StageDpi { index },
            EditAction::StageColor(index) => Self::StageColor { index },
            EditAction::AddStage => Self::AddStage,
            EditAction::ZoneColor(zone) => Self::ZoneColor { zone: zone.into() },
        }
    }
}
impl SavedAction {
    fn restore(self, profile: &SoftwareProfile) -> Result<EditAction> {
        Ok(match self {
            Self::Section { section } => EditAction::Section(section.into()),
            Self::Open => EditAction::OpenRecovered,
            Self::SaveAs => EditAction::SaveAs,
            Self::Resolve => EditAction::Resolve,
            Self::Omit => EditAction::Omit,
            Self::ImportMacro => EditAction::ImportMacro,
            Self::ProfileName => EditAction::ProfileName,
            Self::StageDpi { index } => EditAction::StageDpi(index),
            Self::StageColor { index } => EditAction::StageColor(index),
            Self::AddStage => EditAction::AddStage,
            Self::ZoneColor { zone } => {
                let zone = device_descriptor(&profile.device)
                    .and_then(|device| {
                        device
                            .capabilities
                            .lighting_zones
                            .iter()
                            .find(|value| value.id == zone)
                    })
                    .context("local editor snapshot names an unknown lighting zone")?;
                EditAction::ZoneColor(zone.id)
            }
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum SavedModal {
    Editor {
        title: String,
        action: SavedAction,
        editor: EditorRecovery,
    },
    Macro {
        editor: MacroRecovery,
    },
}
impl SavedModal {
    fn capture(modal: Option<&Modal>) -> Option<Self> {
        match modal? {
            Modal::Editor {
                title,
                action,
                editor,
                ..
            } => Some(Self::Editor {
                title: title.clone(),
                action: (*action).into(),
                editor: editor.recovery(),
            }),
            Modal::Macro(editor) => Some(Self::Macro {
                editor: editor.recovery(),
            }),
            _ => None,
        }
    }
    fn restore(self, profile: &SoftwareProfile) -> Result<Modal> {
        match self {
            Self::Editor {
                title,
                action,
                editor,
            } => Ok(Modal::Editor {
                title,
                action: action.restore(profile)?,
                editor: Editor::from_recovery(editor)?,
                error: None,
            }),
            Self::Macro { editor } => {
                Ok(Modal::Macro(Box::new(MacroEditor::from_recovery(editor)?)))
            }
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Self::Editor { .. } => "unfinished field",
            Self::Macro { .. } => "unfinished macro timeline",
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocalRecord {
    version: u32,
    source_profile: SoftwareProfile,
    source_path: Option<PathBuf>,
    modal: SavedModal,
}
impl LocalRecord {
    pub(crate) fn profile_name(&self) -> &str {
        &self.source_profile.name
    }
    pub(crate) fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
    pub(crate) fn kind(&self) -> &'static str {
        self.modal.label()
    }
    pub(crate) fn matches(&self, document: &ProfileDocument) -> Result<bool> {
        Ok(self.source_profile == *document.profile()
            && self.source_path == absolute_path(document.path())?)
    }
    pub(crate) fn restore(self) -> Result<Modal> {
        self.modal.restore(&self.source_profile)
    }
}

fn absolute_path(path: Option<&Path>) -> Result<Option<PathBuf>> {
    path.map(|path| {
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(env::current_dir()?.join(path))
        }
    })
    .transpose()
}

/// Each TUI process owns at most one local-editor snapshot. Replacements are
/// installed before retiring the previous copy; other sessions are untouched.
pub(crate) struct LocalRecoveryStore {
    directory: PathBuf,
    session: String,
    sequence: u64,
    active: Option<PathBuf>,
    last: Option<Vec<u8>>,
}
impl LocalRecoveryStore {
    pub(crate) fn new(directory: &Path) -> Result<Self> {
        // Apply the same private-directory checks as FILE draft recovery.
        let _check = DraftRecoveryStore::new(directory, RecoveryClient::Tui)?;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let nonce = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
        Ok(Self {
            directory: directory.to_path_buf(),
            session: format!("{stamp:032x}-{:08x}-{nonce:016x}", process::id()),
            sequence: 0,
            active: None,
            last: None,
        })
    }
    pub(crate) fn active_path(&self) -> Option<&Path> {
        self.active.as_deref()
    }
    pub(crate) fn capture(
        &mut self,
        document: &ProfileDocument,
        modal: Option<&Modal>,
    ) -> Result<()> {
        let Some(modal) = SavedModal::capture(modal) else {
            return self.clear_owned();
        };
        let record = LocalRecord {
            version: VERSION,
            source_profile: document.profile().clone(),
            source_path: absolute_path(document.path())?,
            modal,
        };
        let encoded = serde_json::to_vec(&record)?;
        if encoded.len() > MAX_LOCAL_BYTES {
            bail!("local editor recovery snapshot exceeds its bounded size");
        }
        if self.last.as_deref() == Some(encoded.as_slice()) {
            return Ok(());
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .context("local recovery sequence exhausted")?;
        let name = format!("local-tui-{}-{:016x}.json", self.session, self.sequence);
        let destination = self.directory.join(name);
        let temporary = destination.with_extension("tmp");
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).with_context(|| {
            format!(
                "cannot create local editor snapshot {}",
                temporary.display()
            )
        })?;
        file.write_all(&encoded)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &destination).with_context(|| {
            format!(
                "cannot finalize local editor snapshot {}",
                destination.display()
            )
        })?;
        let previous = self.active.replace(destination);
        self.last = Some(encoded);
        if let Some(previous) = previous {
            fs::remove_file(&previous).with_context(|| {
                format!(
                    "new local editor snapshot is safe, but old copy {} could not be removed",
                    previous.display()
                )
            })?;
        }
        Ok(())
    }
    pub(crate) fn clear_owned(&mut self) -> Result<()> {
        if let Some(path) = self.active.take() {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    self.active = Some(path.clone());
                    return Err(error).with_context(|| {
                        format!("cannot clear local editor snapshot {}", path.display())
                    });
                }
            }
        }
        self.last = None;
        Ok(())
    }
    pub(crate) fn list(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("local-tui-") && name.ends_with(".json") {
                paths.push(entry.path());
            }
        }
        paths.sort();
        Ok(paths)
    }
    pub(crate) fn load(&self, path: &Path) -> Result<LocalRecord> {
        self.check_candidate(path)?;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_LOCAL_BYTES as u64 {
            bail!("local editor snapshot must be a bounded regular file");
        }
        let mut encoded = Vec::new();
        File::open(path)?
            .take((MAX_LOCAL_BYTES + 1) as u64)
            .read_to_end(&mut encoded)?;
        if encoded.len() > MAX_LOCAL_BYTES {
            bail!("local editor snapshot exceeds its bounded size");
        }
        let record: LocalRecord = serde_json::from_slice(&encoded)
            .with_context(|| format!("cannot parse local editor snapshot {}", path.display()))?;
        if record.version != VERSION {
            bail!("unsupported local editor snapshot version");
        }
        // Validate the complete modal before the caller can adopt the file.
        record.modal.clone().restore(&record.source_profile)?;
        Ok(record)
    }
    pub(crate) fn restore_into(
        &mut self,
        path: &Path,
        document: &ProfileDocument,
    ) -> Result<Modal> {
        if self.active.is_some() {
            bail!("this session already owns a local editor snapshot");
        }
        let record = self.load(path)?;
        if !record.matches(document)? {
            bail!("this unfinished editor belongs to a different FILE draft; open or restore the matching profile first");
        }
        let modal = record.restore()?;
        self.active = Some(path.to_path_buf());
        self.last = None;
        Ok(modal)
    }
    pub(crate) fn discard(&self, path: &Path) -> Result<()> {
        self.check_candidate(path)?;
        if self.active_path() == Some(path) {
            bail!("cannot discard this session's active local editor snapshot");
        }
        if !fs::symlink_metadata(path)?.file_type().is_file() {
            bail!("refusing to discard a non-regular local editor snapshot");
        }
        fs::remove_file(path)
            .with_context(|| format!("cannot discard local editor snapshot {}", path.display()))
    }
    fn check_candidate(&self, path: &Path) -> Result<()> {
        let name = path.file_name().and_then(|name| name.to_str());
        if path.parent() != Some(self.directory.as_path())
            || !name.is_some_and(|name| name.starts_with("local-tui-") && name.ends_with(".json"))
        {
            bail!("path is not a TUI local editor snapshot");
        }
        Ok(())
    }
}
