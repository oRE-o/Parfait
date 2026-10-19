use parfait_core::{Document, LayerMask, MaskCoverage};
use parfait_storage::Store;
use std::{path::PathBuf, sync::Arc};

fn main() -> anyhow::Result<()> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("Pass a new, isolated fixture directory"))?,
    );
    anyhow::ensure!(
        !directory.exists(),
        "Refusing to replace an existing data directory"
    );
    std::fs::create_dir_all(&directory)?;
    let mut document = Document::new(512, 384);
    document.name = "Raster conversion QA".into();
    document.layers[0].visible = false;
    document.layers[1].name = "Gray and color samples".into();
    document.layers[1].mask = Some(LayerMask {
        enabled: false,
        coverage: MaskCoverage::Uniform(128),
    });
    for (offset, pixel) in Arc::make_mut(&mut document.layers[1].pixels)
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        let column = offset % 512;
        let row = offset / 512;
        let gray = ((column / 32) * 17) as u8;
        let color = if row < 192 {
            [gray; 3]
        } else {
            [[255, 0, 0], [0, 255, 0], [0, 0, 255], [200, 100, 50]][column / 128]
        };
        *pixel = [
            color[0],
            color[1],
            color[2],
            if row % 96 < 64 { 255 } else { 128 },
        ];
    }
    document.touch();
    let mut store = Store::open(&directory.join("recovery.parfait"))?;
    store.save(&document)?;
    store.mark_clean()?;
    store.checkpoint()?;
    anyhow::ensure!(
        store.load()?.unwrap().document == document,
        "Fixture roundtrip mismatch"
    );
    std::fs::write(
        directory.join("before.rgba"),
        document.layers[1].pixels.as_slice(),
    )?;
    println!(
        "512x384, 2 layers, disabled Uniform-128 mask, revision {}",
        document.revision
    );
    Ok(())
}
