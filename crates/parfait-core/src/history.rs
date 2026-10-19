use crate::Document;

struct Entry {
    id: u64,
    label: String,
    document: Document,
}
struct Pending {
    label: String,
    before: Document,
}
pub struct HistoryRow<'a> {
    pub id: u64,
    pub label: &'a str,
    pub current: bool,
    pub future: bool,
}
pub struct History {
    entries: Vec<Entry>,
    cursor: usize,
    next_id: u64,
    pending: Option<Pending>,
}
impl Default for History {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            next_id: 1,
            pending: None,
        }
    }
}
impl History {
    pub fn checkpoint(&mut self, document: &Document) {
        self.begin(document, "Edit");
    }
    pub fn begin(&mut self, document: &Document, label: &str) {
        self.commit(document);
        self.pending = Some(Pending {
            label: label.chars().take(160).collect(),
            before: document.clone(),
        });
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
                id: 0,
                label: "Initial state".into(),
                document: pending.before,
            });
        } else {
            self.entries[self.cursor].document = pending.before;
        }
        self.entries.truncate(self.cursor + 1);
        self.entries.push(Entry {
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
        let next_layer_id = document.next_layer_id.max(pending.before.next_layer_id);
        *document = pending.before;
        document.revision = revision;
        document.next_layer_id = next_layer_id;
        true
    }
    pub fn rows(&self) -> impl Iterator<Item = HistoryRow<'_>> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, entry)| HistoryRow {
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
        let Some(index) = self.entries.iter().position(|entry| entry.id == id) else {
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
        *document = self.entries[index].document.clone();
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
