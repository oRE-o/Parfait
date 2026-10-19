use crate::Document;

impl Document {
    pub fn composite_for_export(&self) -> Vec<u8> {
        if !self.layers.iter().any(|layer| layer.draft && layer.visible) {
            return self.composite();
        }
        let mut output = self.clone();
        for layer in &mut output.layers {
            if layer.draft {
                layer.visible = false;
            }
        }
        output.composite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{History, LayerMask, MaskCoverage};
    use std::sync::Arc;

    #[test]
    fn independent_flags_are_edits_but_do_not_change_display_or_shared_pixels() {
        let mut document = Document::new(2, 1);
        document.layers[1].pixels = Arc::new(vec![230, 30, 10, 255, 0, 0, 0, 0]);
        let before = document.clone();
        let pixels = document.composite();
        let mut history = History::default();
        history.begin(&document, "Layer settings");
        document.layers[1].reference = true;
        document.layers[1].draft = true;
        assert!(!document.same_edit_state(&before));
        history.commit(&document);
        assert_eq!(document.composite(), pixels);
        assert!(Arc::ptr_eq(
            &document.layers[1].pixels,
            &before.layers[1].pixels
        ));
        history.undo(&mut document);
        assert!(document.same_edit_state(&before));
        history.redo(&mut document);
        assert!(document.layers[1].reference && document.layers[1].draft);
    }

    #[test]
    fn export_excludes_drafts_without_rebinding_clips_or_mutating_masks() {
        let mut document = Document::new(2, 1);
        document.layers[1].pixels = Arc::new(vec![230, 30, 10, 255, 0, 0, 0, 0]);
        document.layers[1].draft = true;
        document.layers[1].mask = Some(LayerMask {
            enabled: true,
            coverage: MaskCoverage::Uniform(128),
        });
        document.add_layer();
        document.layers[2].pixels = Arc::new(vec![0, 200, 80, 255, 0, 200, 80, 255]);
        document.layers[2].clipped = true;
        let before = document.clone();
        assert_eq!(document.composite_for_export(), vec![255; 8]);
        assert_eq!(document, before);
        assert!(Arc::ptr_eq(
            &document.layers[1].pixels,
            &before.layers[1].pixels
        ));
        document.layers[1].draft = false;
        document.layers[2].draft = true;
        let mut expected = document.clone();
        expected.layers[2].visible = false;
        assert_eq!(document.composite_for_export(), expected.composite());
        document.layers[0].draft = true;
        document.layers[1].draft = true;
        assert_eq!(document.composite_for_export(), vec![0; 8]);
    }
}
