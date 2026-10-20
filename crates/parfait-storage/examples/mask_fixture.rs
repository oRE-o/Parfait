use parfait_core::{Document, LayerMask, MaskCoverage};
use parfait_storage::Store;
use std::{path::PathBuf, sync::Arc};

fn main() -> anyhow::Result<()> {
    let directory =
        PathBuf::from(std::env::args().nth(1).ok_or_else(|| {
            anyhow::anyhow!("Pass a new, isolated fixture directory")
        })?);
    anyhow::ensure!(
        !directory.exists(),
        "Refusing to replace an existing data directory"
    );
    std::fs::create_dir_all(&directory)?;
    let mut document = Document::new(512, 384);
    document.name = "Mask foundation QA".into();
    document.layers[1].name = "Soft radial mask".into();
    let mut radial = Vec::with_capacity(512 * 384);
    for row in 0..384 {
        for column in 0..512 {
            let offset = (row * 512 + column) * 4;
            Arc::make_mut(&mut document.layers[1].pixels)[offset..offset + 4]
                .copy_from_slice(&[40, 130, 230, 255]);
            let distance = ((column as f32 - 256.).powi(2)
                + (row as f32 - 192.).powi(2))
            .sqrt();
            radial.push(((150. - distance) * 12.).clamp(0., 255.) as u8);
        }
    }
    document.layers[1].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Dense(Arc::new(radial)),
    });
    document.add_layer();
    document.layers[2].name = "Masked clipped stripes".into();
    document.layers[2].clipped = true;
    let mut stripes = Vec::with_capacity(512 * 384);
    for row in 0..384 {
        for column in 0..512 {
            let offset = (row * 512 + column) * 4;
            Arc::make_mut(&mut document.layers[2].pixels)[offset..offset + 4]
                .copy_from_slice(&[230, 60, 90, 255]);
            stripes.push(if (row + column) % 64 < 32 { 255 } else { 0 });
        }
    }
    document.layers[2].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Dense(Arc::new(stripes)),
    });
    document.touch();
    let expected = document.composite();
    let mut store = Store::open(&directory.join("recovery.parfait"))?;
    store.save(&document)?;
    store.mark_clean()?;
    store.checkpoint()?;
    let loaded = store
        .load()?
        .ok_or_else(|| anyhow::anyhow!("Missing saved fixture"))?;
    anyhow::ensure!(
        loaded.document == document && loaded.document.composite() == expected,
        "Fixture roundtrip mismatch"
    );
    std::fs::write(directory.join("expected.rgba"), expected)?;
    println!(
        "512x384, 3 layers, 2 dense masks, revision {}, raw RGBA preserved",
        document.revision
    );
    Ok(())
}
