//! Conservative preflight reservations. No pixel copy is needed to plan history eviction.
use crate::{
    app::Parfait,
    document_session::{DocumentId, SessionKey},
};
use parfait_core::BufferCensus;
use std::collections::HashMap;

impl Parfait {
    pub(crate) fn can_reserve_pixel_headroom(&self, additional: u64) -> bool {
        self.pixel_headroom_fits(additional, &HashMap::new())
    }
    fn pixel_headroom_fits(
        &self,
        additional: u64,
        remove: &HashMap<DocumentId, usize>,
    ) -> bool {
        let mut census = BufferCensus::default();
        if let Some(job) = &self.render_cache.composite_job {
            census.add_document(&job.snapshot);
        }
        for layer in &self.render_cache.thumbnail_sources {
            census.add_buffer(&layer.pixels);
            if let Some(mask) = &layer.mask
                && let parfait_core::MaskCoverage::Dense(values) =
                    &mask.coverage
            {
                census.add_buffer(values);
            }
        }
        if let Some(runtime) = &self.recovery_runtime {
            for snapshot in runtime.worker.snapshots() {
                census.add_document(&snapshot);
            }
        }
        for prompt in &self.source_saves.prompts {
            census.add_document(&prompt.request.snapshot);
        }
        if let Some(snapshot) = &self.tabs.pinned_save {
            census.add_document(snapshot);
        }
        if let Some(cached) = self.local_preview_cached() {
            census.add_document(cached);
        }
        if let Some(source) = self.local_preview_source() {
            census.add_document(source);
        }
        if let Some(cached) = self.transform_cached_result() {
            census.add_document(cached);
        }
        if let Some(source) = self.transform_preview_source() {
            census.add_document(source);
        }
        if let Some(draft) = &self.native_dialog {
            census.add_content(&draft.body);
        }
        for session in self.documents.sessions() {
            census.add_document(&session.document);
            if let Some(saved) = &session.project_saved_state {
                census.add_document(saved);
            }
            session.history.visit_after_eviction(
                *remove.get(&session.key.document_id).unwrap_or(&0),
                |doc| census.add_document(doc),
            );
        }
        census
            .bytes()
            .saturating_add(
                self.image_edit
                    .clipboard
                    .as_ref()
                    .map_or(0, |c| c.pixels.len() as u64),
            )
            .saturating_add(self.transform_reserved_bytes())
            .saturating_add(self.local_preview_reservation())
            .saturating_add(
                self.image_edit
                    .dialog
                    .as_ref()
                    .and_then(|d| d.reference_bytes())
                    .unwrap_or(0),
            )
            .saturating_add(
                self.render_cache
                    .composite_job
                    .as_ref()
                    .map_or(0, |job| job.reserved_bytes),
            )
            .saturating_add(self.image_import.reserved_bytes())
            .saturating_add(additional)
            .saturating_add(self.tabs.reserved_pixels())
            .saturating_add(self.source_saves.reserved_pixels())
            .saturating_add(
                self.documents
                    .sessions()
                    .map(|s| s.edit_reservation)
                    .sum::<u64>(),
            )
            <= self.documents.retained_limit
    }
    pub(crate) fn reserve_pixel_headroom(&mut self, additional: u64) -> bool {
        let mut remove = HashMap::new();
        let fits = |remove: &HashMap<_, usize>| {
            self.pixel_headroom_fits(additional, remove)
        };
        if fits(&remove) {
            return true;
        }
        let active = self.documents.active_id();
        let mut candidates = self
            .documents
            .sessions()
            .flat_map(|session| {
                session.history.eviction_order().enumerate().map(
                    move |(index, age)| {
                        (
                            Some(session.key.document_id) == active,
                            age,
                            session.key.document_id,
                            index + 1,
                        )
                    },
                )
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(active, age, _, _)| (*active, *age));
        for (_, _, id, count) in candidates {
            remove.insert(id, count);
            if fits(&remove) {
                // Publish eviction only when the entire reservation can succeed.
                for (id, count) in remove {
                    self.documents
                        .get_mut(id)
                        .unwrap()
                        .history
                        .evict_oldest(count);
                }
                return true;
            }
        }
        false
    }
    pub(crate) fn preflight_edit(
        &mut self,
        target: SessionKey,
        label: &str,
    ) -> bool {
        let Some(session) = self.documents.get_by_key(target) else {
            return false;
        };
        if !self.tabs.enabled {
            return true;
        }
        let doc = &session.document;
        let canvas = u64::from(doc.width) * u64::from(doc.height);
        // Reserve all potential COW buffers plus a full raster/mask/selection growth.
        // Multi-layer duplication can at most double the current logical allocation.
        let lower = label.to_ascii_lowercase();
        let growth = if label.starts_with("Brush:")
            || label.starts_with("Mask brush:")
        {
            0
        } else if label == "New layer"
            || label == "Import image layer"
            || label == "Merge visible to new layer"
        {
            canvas * 4
        } else if label.starts_with("Duplicate layer") {
            let selected = doc
                .layer_selection_subtrees(session.selected_layer_mask())
                .unwrap_or(session.selected_layer_mask());
            doc.layers
                .iter()
                .enumerate()
                .filter(|(index, _)| selected & (1 << index) != 0)
                .map(|(_, layer)| {
                    canvas
                        * (4 * u64::from(
                            layer.kind == parfait_core::LayerKind::Raster,
                        ) + u64::from(layer.mask.is_some()))
                })
                .sum()
        } else if lower.starts_with("create mask")
            || lower.starts_with("mask ")
            || ((lower.contains("selection") || label == "Select all")
                && doc.selection.is_none())
        {
            canvas
        } else {
            0
        };
        if self.documents.logical_bytes().saturating_add(growth)
            > self.documents.logical_limit
        {
            self.notice = Some(
                "Edit exceeds the open-document logical pixel budget.".into(),
            );
            return false;
        }
        let reserve = if label.starts_with("Brush:") {
            canvas * 4
        } else if label.starts_with("Mask brush:") {
            canvas
        } else {
            doc.logical_pixel_bytes()
                .saturating_add(growth)
                .saturating_add(canvas * 6)
        };
        if !self.reserve_pixel_headroom(reserve) {
            self.notice = Some("Not enough retained pixel budget to preserve undo for this edit.".into());
            return false;
        }
        self.documents.get_by_key_mut(target).unwrap().edit_reservation =
            reserve;
        true
    }
}

impl Parfait {
    pub(crate) fn navigate_history(
        &mut self,
        target: SessionKey,
        action: impl FnOnce(
            &mut parfait_core::History,
            &mut parfait_core::Document,
        ) -> bool,
    ) -> bool {
        let logical = self.documents.logical_bytes();
        let limit = self.documents.logical_limit;
        let Some(session) = self.documents.get_by_key_mut(target) else {
            return false;
        };
        if !self.tabs.enabled {
            return action(&mut session.history, &mut session.document);
        }
        let mut history = session.history.clone();
        let mut document = session.document.clone();
        if !action(&mut history, &mut document) {
            return false;
        }
        let logical = logical - session.document.logical_pixel_bytes()
            + document.logical_pixel_bytes();
        if logical > limit {
            self.notice = Some(
                "History state exceeds the open-document logical pixel budget."
                    .into(),
            );
            return false;
        }
        session.history = history;
        session.document = document;
        true
    }
}
