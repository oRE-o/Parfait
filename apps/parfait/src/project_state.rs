use crate::{app::Parfait, document_session::SessionKey};
use parfait_core::Document;
use parfait_storage::Store;
use std::path::{Path, PathBuf};

impl Parfait {
    pub(crate) fn project_is_modified(&self) -> bool {
        if self.documents.is_empty() {
            return false;
        }
        let session = &*self.documents.active_session;
        session.project.is_none() || session.modified()
    }
    pub(crate) fn project_status(&self) -> &'static str {
        if self.documents.is_empty() {
            return "열린 그림 없음";
        }
        let session = &*self.documents.active_session;
        if session.project.is_none() {
            "파일 저장 필요"
        } else if session.modified() {
            "파일 수정됨"
        } else {
            "파일 저장됨"
        }
    }
    pub(crate) fn recovery_status(&self) -> &'static str {
        if self.documents.is_empty() {
            return "열린 그림 없음";
        }
        if self.tabs.enabled && self.recovery_runtime.is_none() {
            return "개발용: 자동 복구 꺼짐";
        }
        if self.recovery_runtime.is_some()
            && self.documents.active_session.pristine()
        {
            return "미편집: 복구 등록 전";
        }
        let session = &*self.documents.active_session;
        if self.recovery_runtime.is_some() {
            return if session.recovery.failed {
                "복구 저장 실패"
            } else if session.recovery.registered
                && session.recovery.generation.is_some()
                && session.recovery.saved_state
                    == Some(session.history.state_id())
                && session.recovery.saved_request == session.recovery.request_id
                && session.recovery.saved >= session.document.revision
            {
                "복구 저장됨"
            } else {
                "복구 저장 중"
            };
        }
        if session.recovery.failed {
            "복구 저장 실패"
        } else if session.recovery.saved >= session.document.revision {
            "복구 저장됨"
        } else {
            "복구 저장 중"
        }
    }
    pub(crate) fn project_path_allowed(
        &self,
        path: &Path,
    ) -> anyhow::Result<()> {
        let target = self
            .documents
            .active_key()
            .ok_or_else(|| anyhow::anyhow!("No drawing open"))?;
        self.project_path_allowed_for(target, path)
    }
    fn project_path_allowed_for(
        &self,
        owner: SessionKey,
        path: &Path,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.documents.get_by_key(owner).is_some(),
            "Document session is no longer available"
        );
        let target = crate::project_paths::allowed(
            path,
            self.recovery_path.parent().unwrap(),
        )?;
        for session in self.documents.sessions() {
            if session.key != owner {
                anyhow::ensure!(
                    !session.project.as_ref().is_some_and(|path| {
                        crate::project_paths::same_source(path, &target)
                    }),
                    "Another tab owns this source"
                );
            }
        }
        anyhow::ensure!(
            !self
                .source_saves
                .pending
                .values()
                .any(|p| crate::project_paths::same_source(&p.path, &target)),
            "This target is reserved by an original save"
        );
        anyhow::ensure!(
            !self.source_saves.prompts.iter().any(|p| {
                crate::project_paths::same_source(&p.request.path, &target)
            }),
            "This target is awaiting overwrite approval"
        );
        anyhow::ensure!(
            !self
                .tabs
                .pending
                .iter()
                .any(|p| crate::project_paths::same_source(&p.path, &target)),
            "This target is reserved by a file open"
        );
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

    #[cfg(test)]
    pub(crate) fn save_project_to(
        &mut self,
        path: PathBuf,
    ) -> anyhow::Result<()> {
        let target = self
            .documents
            .active_key()
            .ok_or_else(|| anyhow::anyhow!("No drawing open"))?;
        self.save_project_to_for(target, path)?;
        self.wait_source_saves()
    }
    pub(crate) fn save_project_to_for(
        &mut self,
        target: SessionKey,
        path: PathBuf,
    ) -> anyhow::Result<()> {
        let session = self.documents.get_by_key(target).ok_or_else(|| {
            anyhow::anyhow!("Document session is no longer available")
        })?;
        anyhow::ensure!(
            session.editing.stroke.is_none(),
            "Finish the current stroke before saving"
        );
        self.project_path_allowed_for(target, &path)?;
        self.finish_layer_edit_for(target);
        if self.recovery_runtime.is_some() {
            return self.queue_source_save(target, path);
        }
        if self.tabs.enabled {
            anyhow::ensure!(
                self.reserve_pixel_headroom(512 * 1024 * 1024),
                "Not enough pixel budget to validate the previous source snapshot before saving"
            );
        }
        let session = self.documents.get_by_key_mut(target).unwrap();
        let mut candidate = session.document.clone();
        if let Some(name) = path.file_stem().and_then(|name| name.to_str())
            && candidate.name != name
        {
            candidate.name = name.to_owned();
            candidate.touch();
        }
        self.tabs.pinned_save = Some(candidate.clone());
        let saved = Self::write_project_document(&candidate, &path);
        self.tabs.pinned_save = None;
        saved?;
        session.document = candidate;
        session.project = Some(path);
        session.project_saved_state = Some(session.document.clone());
        self.force_save_for(target);
        Ok(())
    }

    pub(crate) fn open_project_from(
        &mut self,
        path: PathBuf,
    ) -> anyhow::Result<()> {
        if self.tabs.enabled {
            return self.queue_project_open(path);
        }
        anyhow::ensure!(
            self.documents.active_session.editing.stroke.is_none(),
            "Finish the current stroke before opening a project"
        );
        self.project_path_allowed(&path)?;
        let mut document = Store::open_read_only(&path)?
            .load()?
            .map(|loaded| loaded.document)
            .ok_or_else(|| anyhow::anyhow!("Project contains no snapshots"))?;
        self.finish_layer_edit();
        let source = &*self.documents.active_session;
        Self::write_project_document(
            &source.document,
            &self.recovery_path.with_file_name("before-open.parfait"),
        )?;
        document.revision =
            source.document.revision.checked_add(1).ok_or_else(|| {
                anyhow::anyhow!("Project revision limit reached")
            })?;
        let mut session =
            crate::document_session::DocumentSession::new(document)?;
        session.project = Some(path);
        session.project_saved_state = Some(session.document.clone());
        self.documents.replace_single(session);
        self.force_save();
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod benchmarks;
