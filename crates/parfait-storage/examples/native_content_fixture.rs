//! Offline example artwork for cache/format and rendered-workspace inspection.
use parfait_core::{
    Affine, Document, LayerContent, TextContent, VectorContent, VectorPath,
};
use parfait_storage::Store;
use std::{path::PathBuf, sync::Arc};
fn main() -> anyhow::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("Provide a new .parfait output path"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Reserve a new destination; never replace an existing artwork file.
    std::fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
    let mut doc = Document::new_with_paper(960, 600);
    doc.name = "편집 기능 검사".into();
    doc.set_solid_layer_color(0, [246, 243, 236]);
    let font = Arc::new(
        include_bytes!(
            "../../../apps/parfait/assets/pretendard/Pretendard-Regular.otf"
        )
        .to_vec(),
    );
    for (text, size, y, color) in [
        ("Parfait · 편집", 48., 60., [36, 65, 65, 255]),
        ("텍스트  /  벡터  /  정렬", 26., 130., [81, 108, 103, 255]),
        ("로컬 저장과 실행 취소", 24., 475., [81, 108, 103, 255]),
    ] {
        doc.create_content_layer(
            LayerContent::Text(TextContent {
                text: text.into(),
                font: font.clone(),
                size,
                color,
                matrix: Affine::translation(65., y),
            }),
            text.into(),
        )
        .map_err(anyhow::Error::msg)?;
    }
    let paths = vec![
        VectorPath {
            points: Arc::new(vec![
                [0., 110.],
                [150., 20.],
                [300., 110.],
                [450., 20.],
                [600., 110.],
                [740., 20.],
            ]),
            color: [63, 149, 145, 255],
            width: 18.,
            closed: false,
            fill: false,
        },
        VectorPath {
            points: Arc::new(vec![
                [0., 175.],
                [120., 175.],
                [120., 205.],
                [0., 205.],
            ]),
            color: [230, 177, 122, 255],
            width: 2.,
            closed: true,
            fill: true,
        },
        VectorPath {
            points: Arc::new(vec![
                [155., 175.],
                [390., 175.],
                [390., 205.],
                [155., 205.],
            ]),
            color: [132, 177, 162, 255],
            width: 2.,
            closed: true,
            fill: true,
        },
        VectorPath {
            points: Arc::new(vec![
                [425., 175.],
                [740., 175.],
                [740., 205.],
                [425., 205.],
            ]),
            color: [88, 123, 121, 255],
            width: 2.,
            closed: true,
            fill: true,
        },
    ];
    doc.create_content_layer(
        LayerContent::Vector(VectorContent {
            paths,
            matrix: Affine::translation(70., 220.),
        }),
        "편집 가능한 경로".into(),
    )
    .map_err(anyhow::Error::msg)?;
    if std::env::args_os().skip(2).any(|a| a == "--raster") {
        let pixels = doc.composite();
        let mut flat = Document::new(doc.width, doc.height);
        flat.name = doc.name;
        flat.layers[1].name = "변형 검사 그림".into();
        flat.layers[1].pixels = Arc::new(pixels);
        doc = flat;
    }
    assert!(doc.validate());
    let digest = doc.art_digest_v1();
    let expected = doc.composite();
    let mut store = Store::open(&path)?;
    store.save(&doc)?;
    let loaded = store.load()?.unwrap().document;
    assert_eq!(loaded.art_digest_v1(), digest);
    assert_eq!(loaded.composite(), expected);
    println!(
        "Artwork fixture ({} editable sources): {}",
        doc.contents.len(),
        path.display()
    );
    Ok(())
}
