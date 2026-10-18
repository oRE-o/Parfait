use crate::Document;
/// Bounded full-state history with shared pixel buffers. Storage revisions stay monotonic.
#[derive(Default)]
pub struct History {
    undo: Vec<Document>,
    redo: Vec<Document>,
}
impl History {
    pub fn checkpoint(&mut self, doc: &Document) {
        self.undo.push(doc.clone());
        self.redo.clear();
        let bytes = doc.width as usize * doc.height as usize * 4 * doc.layers.len();
        let limit = (128 * 1024 * 1024 / bytes.max(1)).clamp(1, 30);
        while self.undo.len() > limit {
            self.undo.remove(0);
        }
    }
    pub fn undo(&mut self, doc: &mut Document) {
        if let Some(mut previous) = self.undo.pop() {
            previous.revision = doc.revision + 1;
            self.redo.push(doc.clone());
            *doc = previous;
        }
    }
    pub fn redo(&mut self, doc: &mut Document) {
        if let Some(mut next) = self.redo.pop() {
            next.revision = doc.revision + 1;
            self.undo.push(doc.clone());
            *doc = next;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
