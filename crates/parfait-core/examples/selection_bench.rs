#[path = "selection_bench/workload.rs"]
mod workload;
use parfait_core::MaskCoverage;
use std::sync::Arc;

fn main() {
    if cfg!(debug_assertions) {
        panic!("Use --release");
    }
    let selected = std::env::args().any(|argument| argument == "--selected");
    let mut document = workload::fixture();
    if selected {
        document.set_selection(Some(MaskCoverage::Dense(Arc::new(
            (0..512 * 512)
                .map(|index| if index % 3 == 0 { 0 } else { 128 })
                .collect(),
        ))));
    }
    println!("operation,opacity_mode,p50_ms,p95_ms,scratch_bytes,checksum");
    workload::run(&document, |sample| {
        if document.selection.is_some() {
            for index in 0..512 * 512 {
                if document.selection_value(index) == 0 {
                    assert_eq!(
                        &sample.layers[1].pixels[index * 4..index * 4 + 4],
                        &document.layers[1].pixels[index * 4..index * 4 + 4]
                    );
                }
            }
        }
    });
}
