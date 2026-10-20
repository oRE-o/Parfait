//! Encoding work buffers are separate from retained document buffers and RSS.
use std::sync::{Mutex, MutexGuard};

const ENCODER_LIMIT: u64 = 1152 * 1024 * 1024;
static ENCODER: Mutex<()> = Mutex::new(());

pub(crate) struct EncodingReservation {
    _encoder: MutexGuard<'static, ()>,
    pub raw: usize,
    pub compressed: usize,
}
impl EncodingReservation {
    pub fn acquire(raw: u64) -> anyhow::Result<Self> {
        Self::with_limit(raw, ENCODER_LIMIT)
    }
    fn with_limit(raw: u64, limit: u64) -> anyhow::Result<Self> {
        let compressed = compressed_bound(raw)
            .ok_or_else(|| anyhow::anyhow!("Encoding size overflow"))?;
        anyhow::ensure!(
            raw.checked_add(compressed).is_some_and(|n| n <= limit),
            "Encoding work budget exceeded before allocation"
        );
        Ok(Self {
            _encoder: ENCODER
                .lock()
                .map_err(|_| anyhow::anyhow!("Encoder lock poisoned"))?,
            raw: raw.try_into()?,
            compressed: compressed.try_into()?,
        })
    }
}
pub(crate) fn compressed_bound(raw: u64) -> Option<u64> {
    // Conservative bound for the supported zlib encoder, including stream overhead.
    raw.checked_add(raw / 8)?.checked_add(256)
}

/// The decoder uses the same work-buffer gate as the encoder.
/// This bound excludes decoded pixel allocations (reserved by the application).
pub(crate) fn decode_bounded(
    payload: &[u8],
    limit: u64,
) -> anyhow::Result<Vec<u8>> {
    use std::io::Read;
    let mut decoder = flate2::read::ZlibDecoder::new(payload);
    let mut raw = Vec::new();
    let mut chunk = [0; 64 * 1024];
    loop {
        let count = decoder.read(&mut chunk)?;
        if count == 0 {
            return Ok(raw);
        }
        let next = raw
            .len()
            .checked_add(count)
            .ok_or_else(|| anyhow::anyhow!("Snapshot length overflow"))?;
        anyhow::ensure!(next as u64 <= limit, "Snapshot too large");
        if next > raw.capacity() {
            let capacity = next.next_power_of_two().min(limit as usize);
            raw.try_reserve_exact(capacity - raw.len())?;
        }
        raw.extend_from_slice(&chunk[..count]);
    }
}

/// A preallocated writer that refuses growth beyond its reservation.
pub(crate) struct WorkBuffer {
    bytes: Vec<u8>,
    limit: usize,
}
impl WorkBuffer {
    pub fn new(limit: usize) -> anyhow::Result<Self> {
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(limit)?;
        Ok(Self { bytes, limit })
    }
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
impl std::io::Write for WorkBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit - self.bytes.len() {
            return Err(std::io::Error::other(
                "Encoding exceeded its reserved workspace",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multi_document_encoder_preflight_and_release() {
        assert!(EncodingReservation::with_limit(1024, 2431).is_err());
        drop(EncodingReservation::with_limit(1024, 2432).unwrap());
        assert!(EncodingReservation::acquire(u64::MAX).is_err());
        assert!(EncodingReservation::acquire(ENCODER_LIMIT).is_err());
        let lease = EncodingReservation::acquire(1024).unwrap();
        assert_eq!(lease.raw, 1024);
        assert_eq!(lease.compressed, 1408);
        assert!(ENCODER.try_lock().is_err());
        drop(lease);
        assert!(EncodingReservation::acquire(1024).is_ok());
    }
    #[test]
    fn multi_document_work_buffers_reject_before_growth_and_bound_decoder_capacity()
     {
        use std::io::Write;
        let mut buffer = WorkBuffer::new(31).unwrap();
        buffer.write_all(&[7; 31]).unwrap();
        assert!(buffer.write_all(&[8]).is_err());
        assert_eq!(buffer.finish(), vec![7; 31]);
        let mut encoder = flate2::write::ZlibEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        );
        encoder.write_all(&[9; 31]).unwrap();
        let compressed = encoder.finish().unwrap();
        assert!(decode_bounded(&compressed, 30).is_err());
        let decoded = decode_bounded(&compressed, 31).unwrap();
        assert_eq!(decoded, vec![9; 31]);
        assert!(decoded.capacity() <= 31);
    }
}
