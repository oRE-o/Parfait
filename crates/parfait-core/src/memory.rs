//! Counts retained allocations, not logical pixels or total process RSS.
use crate::{Document, MaskCoverage};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub struct BufferCensus {
    buffers: HashMap<usize, u64>,
}
impl BufferCensus {
    pub fn add_buffer(&mut self, buffer: &Arc<Vec<u8>>) {
        self.buffers
            .entry(Arc::as_ptr(buffer) as usize)
            .or_insert(buffer.capacity() as u64);
    }
    pub fn add_document(&mut self, doc: &Document) {
        for layer in &doc.layers {
            self.add_buffer(&layer.pixels);
            if let Some(mask) = &layer.mask {
                self.add_coverage(&mask.coverage);
            }
        }
        if let Some(selection) = &doc.selection {
            self.add_coverage(selection);
        }
    }
    fn add_coverage(&mut self, coverage: &MaskCoverage) {
        if let MaskCoverage::Dense(buffer) = coverage {
            self.add_buffer(buffer);
        }
    }
    pub fn bytes(&self) -> u64 {
        self.buffers.values().copied().sum()
    }
}

/// Decode the existing byte-sequence wire representation without Vec's geometric
/// spare capacity. Binary readers can borrow the payload and allocate exactly once.
#[doc(hidden)]
pub fn deserialize_pixel_buffer<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Arc<Vec<u8>>, D::Error> {
    struct Bytes;
    impl<'de> serde::de::Visitor<'de> for Bytes {
        type Value = Arc<Vec<u8>>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a bounded pixel byte sequence")
        }
        fn visit_bytes<E: serde::de::Error>(
            self,
            value: &[u8],
        ) -> Result<Self::Value, E> {
            if value.len() > 4096 * 4096 * 4 {
                return Err(E::custom("Pixel buffer exceeds canvas limit"));
            }
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(value.len()).map_err(E::custom)?;
            bytes.extend_from_slice(value);
            Ok(Arc::new(bytes))
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            use serde::de::Error;
            let mut bytes = Vec::new();
            while let Some(byte) = seq.next_element::<u8>()? {
                if bytes.len() >= 4096 * 4096 * 4 {
                    return Err(A::Error::custom(
                        "Pixel buffer exceeds canvas limit",
                    ));
                }
                bytes.push(byte);
            }
            bytes.shrink_to_fit();
            Ok(Arc::new(bytes))
        }
    }
    deserializer.deserialize_bytes(Bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{History, LayerMask};
    #[test]
    fn multi_document_unique_pixel_mask_selection_capacity_and_history() {
        let mut a = Document::new(4, 4);
        let mut mask = Vec::with_capacity(32);
        mask.resize(16, 128);
        let mask = Arc::new(mask);
        a.layers[1].mask = Some(LayerMask {
            enabled: true,
            coverage: MaskCoverage::Dense(mask.clone()),
        });
        a.selection = Some(MaskCoverage::Dense(mask));
        let expected =
            a.layers.iter().map(|l| l.pixels.capacity() as u64).sum::<u64>()
                + 32;
        let mut history = History::default();
        history.begin(&a, "Pixel");
        let mut b = a.clone();
        Arc::make_mut(&mut b.layers[1].pixels)[0] = 22;
        history.commit(&b);
        let mut census = BufferCensus::default();
        census.add_document(&a);
        census.add_document(&a.clone());
        assert_eq!(census.bytes(), expected);
        census.add_document(&b);
        history.visit_documents(|d| census.add_document(d));
        assert_eq!(census.bytes(), expected + 64);
        let mut uniform = a.clone();
        uniform.selection = Some(MaskCoverage::Uniform(128));
        census.add_document(&uniform);
        assert_eq!(census.bytes(), expected + 64);
    }
}
