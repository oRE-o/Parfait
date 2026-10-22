//! Pixel-only image insertion. File decoding stays outside the reusable core.
use crate::Document;
use std::sync::Arc;

impl Document {
    /// Create an unbound image document, retaining its alpha over hidden paper.
    pub fn from_rgba_image(
        name: String,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Option<Self> {
        if width == 0
            || height == 0
            || width > 4096
            || height > 4096
            || pixels.len() != width as usize * height as usize * 4
        {
            return None;
        }
        let mut document = Self::new_background(width, height, true);
        document.name = name.clone();
        document.layers[0].visible = false;
        document.layers[0].locked = true;
        document.layers[1].name = name;
        document.layers[1].pixels = Arc::new(pixels);
        Some(document)
    }

    /// Insert an unscaled, centered RGBA image above the active layer.
    /// Oversized images are rejected rather than silently cropped.
    pub fn import_raster_layer(
        &mut self,
        name: String,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> bool {
        if width == 0
            || height == 0
            || width > 4096
            || height > 4096
            || width > self.width
            || height > self.height
            || pixels.len() != width as usize * height as usize * 4
        {
            return false;
        }
        let count = self.layers.len();
        self.add_layer();
        if self.layers.len() != count + 1 {
            return false;
        }
        let layer = &mut self.layers[self.active];
        layer.name = name;
        layer.clipped = false;
        let destination = Arc::get_mut(&mut layer.pixels).unwrap();
        let left = (self.width - width) as usize / 2;
        let top = (self.height - height) as usize / 2;
        let stride = self.width as usize * 4;
        for y in 0..height as usize {
            let start = (top + y) * stride + left * 4;
            let source = y * width as usize * 4;
            destination[start..start + width as usize * 4]
                .copy_from_slice(&pixels[source..source + width as usize * 4]);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_document_preserves_alpha_without_allocating_paper_pixels() {
        let pixels = vec![40, 80, 120, 128, 0, 0, 0, 0];
        let doc =
            Document::from_rgba_image("Image".into(), 2, 1, pixels.clone())
                .unwrap();
        assert!(doc.validate());
        assert_eq!(*doc.layers[1].pixels, pixels);
        assert!(doc.layers[0].pixels.is_empty());
        assert!(!doc.layers[0].visible);
        assert_eq!(doc.logical_pixel_bytes(), 8);
        assert_eq!(doc.composite_for_export(), pixels);
        assert!(
            Document::from_rgba_image("Bad".into(), 0, 1, vec![]).is_none()
        );
        assert!(
            Document::from_rgba_image("Bad".into(), 4097, 1, vec![]).is_none()
        );
        assert!(
            Document::from_rgba_image("Bad".into(), 1, 1, vec![0; 3]).is_none()
        );
    }
    #[test]
    fn insertion_centers_exact_rgba_and_preserves_existing_pixels() {
        let mut doc = Document::new(6, 4);
        let before = doc.clone();
        let pixels = [90, 60, 30, 128].repeat(4);
        assert!(doc.import_raster_layer("Imported".into(), 2, 2, &pixels));
        assert!(doc.validate());
        assert_eq!(&doc.layers[..2], &before.layers);
        let layer = &doc.layers[doc.active];
        assert_eq!(layer.name, "Imported");
        for y in 0..4 {
            for x in 0..6 {
                let at = (y * 6 + x) * 4;
                let expected = if (2..4).contains(&x) && (1..3).contains(&y) {
                    [90, 60, 30, 128]
                } else {
                    [0; 4]
                };
                assert_eq!(layer.pixels[at..at + 4], expected);
            }
        }
    }
    #[test]
    fn invalid_oversized_and_locked_folder_insertions_are_atomic() {
        let mut doc = Document::new(4, 4);
        for (w, h, pixels) in
            [(5, 1, vec![0; 20]), (1, 0, vec![]), (2, 2, vec![0; 15])]
        {
            let before = doc.clone();
            assert!(!doc.import_raster_layer("Bad".into(), w, h, &pixels));
            assert_eq!(doc, before);
        }
        doc.create_layer_folder("Locked".into());
        doc.layers[doc.active].locked = true;
        let before = doc.clone();
        assert!(!doc.import_raster_layer("Bad".into(), 1, 1, &[0; 4]));
        assert_eq!(doc, before);
    }
}
