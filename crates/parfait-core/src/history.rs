use crate::{Document, EditStateId};
use std::sync::atomic::{AtomicU64, Ordering};
static ENTRY_ORDER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
struct Entry {
    order: u64,
    edit_state: EditStateId,
    id: u64,
    label: String,
    document: Document,
}
#[derive(Clone)]
struct Pending {
    before_id: EditStateId,
    reserved_id: EditStateId,
    label: String,
    before: Document,
}
pub struct HistoryRow<'a> {
    pub id: u64,
    pub label: &'a str,
    pub current: bool,
    pub future: bool,
}
#[derive(Clone)]
pub struct History {
    current_state: EditStateId,
    entries: Vec<Entry>,
    cursor: usize,
    next_id: u64,
    pending: Option<Pending>,
}
impl Default for History {
    fn default() -> Self {
        Self::try_new().expect("OS entropy unavailable for history")
    }
}
impl History {
    pub fn try_new() -> Result<Self, getrandom::Error> {
        Ok(Self {
            current_state: EditStateId::try_new()?,
            entries: Vec::new(),
            cursor: 0,
            next_id: 1,
            pending: None,
        })
    }
    pub fn restored(state: EditStateId) -> Self {
        Self {
            current_state: state,
            entries: Vec::new(),
            cursor: 0,
            next_id: 1,
            pending: None,
        }
    }
    /// Includes pending before images; callers must not evict those.
    pub fn visit_documents(&self, mut visit: impl FnMut(&Document)) {
        for entry in &self.entries {
            visit(&entry.document);
        }
        if let Some(pending) = &self.pending {
            visit(&pending.before);
        }
    }
    /// Visits protected images and a hypothetical history after oldest-first eviction.
    pub fn visit_after_eviction(
        &self,
        remove: usize,
        mut visit: impl FnMut(&Document),
    ) {
        for entry in self.entries.iter().skip(remove.min(self.evictable())) {
            visit(&entry.document);
        }
        if let Some(pending) = &self.pending {
            visit(&pending.before);
        }
    }
    pub fn eviction_order(&self) -> impl Iterator<Item = u64> + '_ {
        self.entries.iter().take(self.evictable()).map(|entry| entry.order)
    }
    pub fn evictable(&self) -> usize {
        self.cursor.saturating_sub(1)
    }
    pub fn evict_oldest(&mut self, count: usize) {
        let count = count.min(self.evictable());
        self.entries.drain(..count);
        self.cursor -= count;
    }
    pub fn pending_changed(&self, document: &Document) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|p| !p.before.same_edit_state(document))
    }
    pub fn state_id(&self) -> EditStateId {
        self.current_state
    }
    pub fn checkpoint(&mut self, document: &Document) {
        self.begin(document, "Edit");
    }
    pub fn begin(&mut self, document: &Document, label: &str) {
        self.try_begin(document, label)
            .expect("OS entropy unavailable for edit");
    }
    pub fn try_begin(
        &mut self,
        document: &Document,
        label: &str,
    ) -> Result<(), getrandom::Error> {
        let reserved = loop {
            let candidate = EditStateId::try_new()?;
            if candidate != self.current_state
                && self
                    .entries
                    .iter()
                    .all(|entry| entry.edit_state != candidate)
                && self
                    .pending
                    .as_ref()
                    .is_none_or(|pending| pending.reserved_id != candidate)
            {
                break candidate;
            }
        };
        self.try_begin_with(document, label, || Ok(reserved))
    }
    fn try_begin_with(
        &mut self,
        document: &Document,
        label: &str,
        create: impl FnOnce() -> Result<EditStateId, getrandom::Error>,
    ) -> Result<(), getrandom::Error> {
        // Reserve before mutation; publish only when a real edit commits.
        let reserved_id = create()?;
        self.commit(document);
        self.pending = Some(Pending {
            before_id: self.current_state,
            reserved_id,
            label: label.chars().take(160).collect(),
            before: document.clone(),
        });
        Ok(())
    }
    pub fn commit(&mut self, document: &Document) -> bool {
        let Some(pending) = self.pending.take() else {
            return false;
        };
        if pending.before.same_edit_state(document) {
            return false;
        }
        if self.entries.is_empty() {
            self.entries.push(Entry {
                order: ENTRY_ORDER.fetch_add(1, Ordering::Relaxed),
                edit_state: pending.before_id,
                id: 0,
                label: "Initial state".into(),
                document: pending.before,
            });
        } else {
            self.entries[self.cursor].document = pending.before;
        }
        self.entries.truncate(self.cursor + 1);
        self.current_state = pending.reserved_id;
        self.entries.push(Entry {
            order: ENTRY_ORDER.fetch_add(1, Ordering::Relaxed),
            edit_state: self.current_state,
            id: self.next_id,
            label: pending.label,
            document: document.clone(),
        });
        self.next_id += 1;
        let bytes = document.logical_pixel_bytes();
        let limit = (128 * 1024 * 1024 / bytes.max(1)).clamp(1, 30) as usize;
        let excess = self.entries.len().saturating_sub(limit + 1);
        if excess > 0 {
            self.entries.drain(..excess);
            self.entries[0].label = "Earlier state".into();
        }
        self.cursor = self.entries.len() - 1;
        true
    }
    pub fn cancel(&mut self, document: &mut Document) -> bool {
        let Some(pending) = self.pending.take() else {
            return false;
        };
        if pending.before.same_edit_state(document) {
            return false;
        }
        let revision = document.revision + 1;
        let next_layer_id =
            document.next_layer_id.max(pending.before.next_layer_id);
        let name = std::mem::take(&mut document.name);
        self.current_state = pending.before_id;
        *document = pending.before;
        document.name = name;
        document.revision = revision;
        document.next_layer_id = next_layer_id;
        true
    }
    pub fn rows(&self) -> impl Iterator<Item = HistoryRow<'_>> {
        self.entries.iter().enumerate().map(|(index, entry)| HistoryRow {
            id: entry.id,
            label: &entry.label,
            current: index == self.cursor,
            future: index > self.cursor,
        })
    }
    pub fn can_undo(&self) -> bool {
        self.cursor > 0 || self.pending.is_some()
    }
    pub fn can_redo(&self) -> bool {
        self.cursor + 1 < self.entries.len()
    }
    pub fn restore(&mut self, document: &mut Document, id: u64) -> bool {
        self.commit(document);
        let Some(index) = self.entries.iter().position(|entry| entry.id == id)
        else {
            return false;
        };
        self.restore_index(document, index)
    }
    fn restore_index(&mut self, document: &mut Document, index: usize) -> bool {
        if index == self.cursor || index >= self.entries.len() {
            return false;
        }
        let revision = document.revision + 1;
        let next_layer_id = document
            .next_layer_id
            .max(self.entries[index].document.next_layer_id);
        let name = std::mem::take(&mut document.name);
        *document = self.entries[index].document.clone();
        document.name = name;
        self.current_state = self.entries[index].edit_state;
        document.revision = revision;
        document.next_layer_id = next_layer_id;
        self.cursor = index;
        true
    }
    pub fn undo(&mut self, document: &mut Document) -> bool {
        self.commit(document);
        self.cursor
            .checked_sub(1)
            .is_some_and(|index| self.restore_index(document, index))
    }
    pub fn redo(&mut self, document: &mut Document) -> bool {
        self.commit(document);
        self.restore_index(document, self.cursor + 1)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn named_rows_share_cursor_keep_future_and_replace_only_real_branches() {
        let mut document = Document::new(4, 4);
        let mut history = History::default();
        for label in ["A", "B"] {
            history.begin(&document, label);
            document.add_layer();
            assert!(history.commit(&document));
        }
        let ids: Vec<_> = history.rows().map(|row| row.id).collect();
        history.undo(&mut document);
        assert!(history.rows().last().unwrap().future);
        history.begin(&document, "No change");
        assert!(!history.commit(&document));
        assert!(history.can_redo());
        assert!(history.restore(&mut document, ids[2]));
        assert_eq!(document.layers.len(), 4);
        history.restore(&mut document, ids[1]);
        history.undo(&mut document);
        assert_eq!(document.layers.len(), 2);
        assert!(history.rows().next().unwrap().current);
        history.restore(&mut document, ids[1]);
        history.begin(&document, "C");
        document.layers[1].opacity = 0.5;
        document.touch();
        history.commit(&document);
        assert!(!history.can_redo());
        assert_eq!(history.rows().last().unwrap().label, "C");
        assert!(!history.restore(&mut document, ids[2]));
    }

    #[test]
    fn cancellation_preserves_future_and_revisions_without_pixel_copies() {
        let mut document = Document::new(4, 4);
        let original = document.clone();
        let mut history = History::default();
        history.begin(&document, "Layer");
        document.add_layer();
        history.commit(&document);
        history.undo(&mut document);
        history.begin(&document, "Draft");
        document.layers[1].opacity = 0.25;
        document.touch();
        let revision = document.revision;
        assert!(history.cancel(&mut document));
        assert!(document.revision > revision);
        assert_eq!(document.layers, original.layers);
        assert!(Arc::ptr_eq(
            &document.layers[1].pixels,
            &original.layers[1].pixels
        ));
        assert!(history.redo(&mut document));
        assert_eq!(document.layers.len(), 3);
    }

    #[test]
    fn history_is_bounded_across_undo_and_redo_and_ignores_selection() {
        let mut document = Document::new(4, 4);
        let pixels = document.layers[1].pixels.clone();
        let mut history = History::default();
        for index in 0..50 {
            history.begin(&document, "Rename");
            document.layers[1].name = index.to_string();
            document.touch();
            history.commit(&document);
        }
        assert_eq!(history.rows().count(), 31);
        assert_eq!(history.rows().next().unwrap().label, "Earlier state");
        while history.undo(&mut document) {}
        assert_eq!(history.rows().count(), 31);
        assert!(history.can_redo());
        history.begin(&document, "Selection");
        document.active = 0;
        document.touch();
        assert!(!history.commit(&document));
        assert!(history.can_redo());
        assert!(Arc::ptr_eq(&document.layers[1].pixels, &pixels));
    }
    #[test]
    fn undo_branch_has_unique_revisions() {
        let mut d = Document::new(4, 4);
        let mut h = History::default();
        h.checkpoint(&d);
        d.add_layer();
        let rev = d.revision;
        h.undo(&mut d);
        assert_eq!(d.layers.len(), 2);
        assert!(d.revision > rev);
        h.redo(&mut d);
        assert_eq!(d.layers.len(), 3);
        h.undo(&mut d);
        h.checkpoint(&d);
        d.add_layer();
        let count = d.layers.len();
        h.redo(&mut d);
        assert_eq!(d.layers.len(), count);
    }
}

#[cfg(test)]
mod multi_document_tests {
    use super::*;
    #[test]
    fn multi_document_edit_ids_follow_history_without_reverting_file_names() {
        let mut d = Document::new(4, 4);
        let mut h = History::try_new().unwrap();
        let initial = h.state_id();
        h.try_begin(&d, "Layer name").unwrap();
        d.layers[1].name = "Edited".into();
        assert!(h.commit(&d));
        let saved = h.state_id();
        assert_ne!(initial, saved);
        h.try_begin(&d, "Opacity").unwrap();
        d.layers[1].opacity = 0.5;
        h.commit(&d);
        let edited = h.state_id();
        d.name = "Saved under new filename".into();
        h.undo(&mut d);
        assert_eq!(h.state_id(), saved);
        assert!(h.can_redo());
        h.try_begin(&d, "File metadata only").unwrap();
        d.name = "Newest filename".into();
        assert!(!h.commit(&d));
        assert_eq!(h.state_id(), saved);
        assert!(h.can_redo());
        h.redo(&mut d);
        assert_eq!(h.state_id(), edited);
        assert_eq!(d.name, "Newest filename");
        h.undo(&mut d);
        h.undo(&mut d);
        assert_eq!(h.state_id(), initial);
        assert_eq!(d.name, "Newest filename");
        h.try_begin(&d, "Cancelled").unwrap();
        d.layers[1].opacity = 0.2;
        h.cancel(&mut d);
        assert_eq!(h.state_id(), initial);
        assert!(h.can_redo());
    }
    #[test]
    fn multi_document_entropy_failure_keeps_pending_state_and_redo() {
        let mut d = Document::new(2, 2);
        let mut h = History::default();
        h.begin(&d, "Layer");
        d.add_layer();
        h.commit(&d);
        h.undo(&mut d);
        let id = h.state_id();
        let before = d.clone();
        assert!(
            h.try_begin_with(&d, "Cannot allocate", || Err(
                getrandom::Error::UNSUPPORTED
            ))
            .is_err()
        );
        assert_eq!(h.state_id(), id);
        assert_eq!(d, before);
        assert!(h.can_redo());

        h.try_begin(&d, "Pending opacity").unwrap();
        d.layers[1].opacity = 0.25;
        let pending_document = d.clone();
        assert!(
            h.try_begin_with(&d, "Cannot replace pending", || {
                Err(getrandom::Error::UNSUPPORTED)
            })
            .is_err()
        );
        assert_eq!(h.state_id(), id);
        assert_eq!(d, pending_document);
        assert!(h.can_redo());
        assert!(h.commit(&d));
        assert_ne!(h.state_id(), id);
        assert!(!h.can_redo());
        assert!(h.undo(&mut d));
        assert_eq!(h.state_id(), id);
        assert_eq!(d.layers[1].opacity, before.layers[1].opacity);
    }
}
