//! Versioned artwork equivalence, not a file checksum or authentication token.
use crate::{BlendMode, Document, LayerKind, MaskCoverage};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtDigestV1(pub [u8; 32]);

fn coverage(hash: &mut Sha256, value: &MaskCoverage, count: usize) {
    hash.update((count as u64).to_le_bytes());
    match value {
        MaskCoverage::Dense(values) => hash.update(values.as_slice()),
        MaskCoverage::Uniform(value) => {
            let chunk = [*value; 4096];
            for _ in 0..count / chunk.len() {
                hash.update(chunk);
            }
            hash.update(&chunk[..count % chunk.len()]);
        }
    }
}

fn blend_tag(mode: BlendMode) -> u8 {
    match mode {
        BlendMode::Normal => 0,
        BlendMode::Multiply => 1,
        BlendMode::Screen => 2,
        BlendMode::Overlay => 3,
        BlendMode::Darken => 4,
        BlendMode::Lighten => 5,
        BlendMode::ColorDodge => 6,
        BlendMode::ColorBurn => 7,
        BlendMode::HardLight => 8,
        BlendMode::SoftLight => 9,
        BlendMode::Difference => 10,
        BlendMode::Exclusion => 11,
        BlendMode::Add => 12,
        BlendMode::Subtract => 13,
        BlendMode::Through => 14,
    }
}

impl Document {
    /// Streams validated logical bytes without allocating a canonical full image.
    pub fn art_digest_v1(&self) -> Option<ArtDigestV1> {
        if !self.validate() {
            return None;
        }
        let mut hash = Sha256::new();
        hash.update(b"Parfait.ArtDigest\0");
        hash.update(1_u32.to_le_bytes());
        hash.update(self.width.to_le_bytes());
        hash.update(self.height.to_le_bytes());
        hash.update((self.layers.len() as u64).to_le_bytes());
        for layer in &self.layers {
            hash.update(layer.id.0.to_le_bytes());
            match layer.kind {
                LayerKind::Raster => hash.update([0]),
                LayerKind::Folder => hash.update([1]),
                LayerKind::Fill(color) => {
                    hash.update([2]);
                    hash.update(color);
                }
                LayerKind::Paper(color) => {
                    hash.update([3]);
                    hash.update(color);
                }
            }
            match layer.parent {
                None => hash.update([0]),
                Some(parent) => {
                    hash.update([1]);
                    hash.update(parent.0.to_le_bytes());
                }
            }
            hash.update([
                layer.visible as u8,
                layer.locked as u8,
                layer.alpha_locked as u8,
                layer.clipped as u8,
                layer.reference as u8,
                layer.draft as u8,
            ]);
            let opacity =
                if layer.opacity == 0. { 0_f32 } else { layer.opacity };
            hash.update(opacity.to_bits().to_le_bytes());
            hash.update([blend_tag(layer.blend_mode)]);
            hash.update((layer.name.len() as u64).to_le_bytes());
            hash.update(layer.name.as_bytes());
            hash.update((layer.pixels.len() as u64).to_le_bytes());
            hash.update(layer.pixels.as_slice());
            match &layer.mask {
                None => hash.update([0]),
                Some(mask) => {
                    hash.update([1, mask.enabled as u8]);
                    coverage(
                        &mut hash,
                        &mask.coverage,
                        self.width as usize * self.height as usize,
                    );
                }
            }
        }
        match &self.selection {
            None => hash.update([0]),
            Some(selection) => {
                hash.update([1]);
                coverage(
                    &mut hash,
                    selection,
                    self.width as usize * self.height as usize,
                );
            }
        }
        if !self.contents.is_empty() {
            use crate::LayerContent;
            fn float(h: &mut Sha256, v: f32) {
                h.update(
                    (if v == 0. { 0_f32 } else { v }).to_bits().to_le_bytes(),
                );
            }
            hash.update(b"Parfait.NativeContent\0");
            hash.update((self.contents.len() as u64).to_le_bytes());
            for node in &self.contents {
                hash.update(node.layer.0.to_le_bytes());
                match node.body().as_ref() {
                    LayerContent::Text(t) => {
                        hash.update([0]);
                        hash.update((t.text.len() as u64).to_le_bytes());
                        hash.update(t.text.as_bytes());
                        hash.update((t.font.len() as u64).to_le_bytes());
                        hash.update(t.font.as_slice());
                        float(&mut hash, t.size);
                        hash.update(t.color);
                    }
                    LayerContent::Vector(v) => {
                        hash.update([1]);
                        hash.update((v.paths.len() as u64).to_le_bytes());
                        for p in &v.paths {
                            hash.update((p.points.len() as u64).to_le_bytes());
                            for point in p.points.iter() {
                                for x in point {
                                    float(&mut hash, *x);
                                }
                            }
                            hash.update(p.color);
                            float(&mut hash, p.width);
                            hash.update([p.closed as u8, p.fill as u8]);
                        }
                    }
                }
                for x in node.body().matrix().0 {
                    float(&mut hash, x);
                }
            }
        }
        if !self.layer_colors.is_empty() {
            hash.update(b"Parfait.LayerColors\0");
            hash.update((self.layer_colors.len() as u64).to_le_bytes());
            for (id, color) in &self.layer_colors {
                hash.update(id.0.to_le_bytes());
                hash.update(color);
            }
        }
        Some(ArtDigestV1(hash.finalize().into()))
    }
}

#[cfg(test)]
mod tests;
