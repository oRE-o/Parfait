use crate::{app::Parfait, canvas::View};
use parfait_core::{Document, History};
use parfait_storage::Store;
use std::path::{Path, PathBuf};

impl Parfait {
    pub(crate) fn project_is_modified(&self) -> bool {
        self.project.is_none()
            || self
                .project_saved_state
                .as_ref()
                .is_none_or(|saved| !self.doc.same_edit_state(saved))
    }

    pub(crate) fn project_status(&self) -> &'static str {
        if self.project.is_none() {
            "파일 저장 필요"
        } else if self.project_is_modified() {
            "파일 수정됨"
        } else {
            "파일 저장됨"
        }
    }

    pub(crate) fn recovery_status(&self) -> &'static str {
        if self.saving_failed {
            "복구 저장 실패"
        } else if self.saved >= self.doc.revision {
            "복구 저장됨"
        } else {
            "복구 저장 중"
        }
    }

    fn project_path_allowed(&self, path: &Path) -> anyhow::Result<()> {
        fn resolved(path: &Path) -> anyhow::Result<PathBuf> {
            match path.symlink_metadata() {
                Ok(_) => return Ok(path.canonicalize()?),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            let parent = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let name = path
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Choose a project file name"))?;
            Ok(parent.canonicalize()?.join(name))
        }
        let target = resolved(path)?;
        for name in [
            "recovery.parfait",
            "before-open.parfait",
            "before-new.parfait",
            "preferences.sqlite",
            "session.lock",
        ] {
            for suffix in ["", "-wal", "-shm", "-journal"] {
                anyhow::ensure!(
                    target
                        != resolved(&self.recovery_path.with_file_name(format!("{name}{suffix}")))?,
                    "Choose a project file outside the application's working files"
                );
            }
        }
        Ok(())
    }

    pub(crate) fn write_project_document(document: &Document, path: &Path) -> anyhow::Result<()> {
        let mut store = Store::open(path)?;
        let mut snapshot = document.clone();
        if let Some(old) = store.load()? {
            snapshot.revision = snapshot
                .revision
                .max(old.document.revision)
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("Project revision limit reached"))?;
        }
        store.save(&snapshot)?;
        store.mark_clean()?;
        store.checkpoint()?;
        Ok(())
    }

    pub(crate) fn save_project_to(&mut self, path: PathBuf) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.stroke.is_none(),
            "Finish the current stroke before saving"
        );
        self.project_path_allowed(&path)?;
        self.finish_layer_edit();
        let mut candidate = self.doc.clone();
        if let Some(name) = path.file_stem().and_then(|name| name.to_str())
            && candidate.name != name
        {
            candidate.name = name.to_owned();
            candidate.touch();
        }
        Self::write_project_document(&candidate, &path)?;
        self.doc = candidate;
        self.project = Some(path);
        self.project_saved_state = Some(self.doc.clone());
        self.force_save();
        Ok(())
    }

    pub(crate) fn open_project_from(&mut self, path: PathBuf) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.stroke.is_none(),
            "Finish the current stroke before opening a project"
        );
        self.project_path_allowed(&path)?;
        self.finish_layer_edit();
        Self::write_project_document(
            &self.doc,
            &self.recovery_path.with_file_name("before-open.parfait"),
        )?;
        let mut document = Store::open(&path)?
            .load()?
            .map(|loaded| loaded.document)
            .ok_or_else(|| anyhow::anyhow!("Project contains no snapshots"))?;
        document.revision = self
            .doc
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Project revision limit reached"))?;
        self.doc = document;
        self.project = Some(path);
        self.project_saved_state = Some(self.doc.clone());
        self.mask_target = None;
        self.mask_overlay = None;
        self.history = History::default();
        self.layer_list = Default::default();
        self.view = View::default();
        self.force_save();
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod benchmarks;
