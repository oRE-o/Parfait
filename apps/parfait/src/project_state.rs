use crate::app::Parfait;
use parfait_core::Document;
use parfait_storage::Store;
use std::path::{Path, PathBuf};

impl Parfait {
    pub(crate) fn project_is_modified(&self) -> bool {
        self.documents.session.project.is_none()
            || self.documents.session.project_saved_state.as_ref().is_none_or(
                |saved| !self.documents.session.document.same_edit_state(saved),
            )
    }

    pub(crate) fn project_status(&self) -> &'static str {
        if self.documents.session.project.is_none() {
            "파일 저장 필요"
        } else if self.project_is_modified() {
            "파일 수정됨"
        } else {
            "파일 저장됨"
        }
    }

    pub(crate) fn recovery_status(&self) -> &'static str {
        if self.documents.session.recovery.failed {
            "복구 저장 실패"
        } else if self.documents.session.recovery.saved
            >= self.documents.session.document.revision
        {
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
                        != resolved(
                            &self
                                .recovery_path
                                .with_file_name(format!("{name}{suffix}"))
                        )?,
                    "Choose a project file outside the application's working files"
                );
            }
        }
        Ok(())
    }

    pub(crate) fn write_project_document(
        document: &Document,
        path: &Path,
    ) -> anyhow::Result<()> {
        let mut store = Store::open(path)?;
        let mut snapshot = document.clone();
        if let Some(old) = store.load()? {
            snapshot.revision = snapshot
                .revision
                .max(old.document.revision)
                .checked_add(1)
                .ok_or_else(|| {
                    anyhow::anyhow!("Project revision limit reached")
                })?;
        }
        store.save(&snapshot)?;
        store.mark_clean()?;
        store.checkpoint()?;
        Ok(())
    }

    pub(crate) fn save_project_to(
        &mut self,
        path: PathBuf,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.documents.session.editing.stroke.is_none(),
            "Finish the current stroke before saving"
        );
        self.project_path_allowed(&path)?;
        self.finish_layer_edit();
        let mut candidate = self.documents.session.document.clone();
        if let Some(name) = path.file_stem().and_then(|name| name.to_str())
            && candidate.name != name
        {
            candidate.name = name.to_owned();
            candidate.touch();
        }
        Self::write_project_document(&candidate, &path)?;
        self.documents.session.document = candidate;
        self.documents.session.project = Some(path);
        self.documents.session.project_saved_state =
            Some(self.documents.session.document.clone());
        self.force_save();
        Ok(())
    }

    pub(crate) fn open_project_from(
        &mut self,
        path: PathBuf,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.documents.session.editing.stroke.is_none(),
            "Finish the current stroke before opening a project"
        );
        self.project_path_allowed(&path)?;
        self.finish_layer_edit();
        Self::write_project_document(
            &self.documents.session.document,
            &self.recovery_path.with_file_name("before-open.parfait"),
        )?;
        let mut document = Store::open(&path)?
            .load()?
            .map(|loaded| loaded.document)
            .ok_or_else(|| anyhow::anyhow!("Project contains no snapshots"))?;
        document.revision = self
            .documents
            .session
            .document
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Project revision limit reached"))?;
        let mut session =
            crate::document_session::DocumentSession::new(document)?;
        session.project = Some(path);
        session.project_saved_state = Some(session.document.clone());
        self.documents.session = session;
        self.force_save();
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod benchmarks;
