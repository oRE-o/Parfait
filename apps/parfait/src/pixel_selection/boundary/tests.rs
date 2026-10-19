use super::*;

fn frame(cache: &mut BoundaryCache, document: &Document, view: &View, context: &egui::Context) {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.))),
            ..Default::default()
        },
        |ui| {
            cache.paint(
                ui,
                document,
                view,
                Rect::from_min_size(egui::pos2(20., 20.), egui::vec2(760., 560.)),
            );
        },
    );
    output.textures_delta.clear();
}

#[test]
fn holes_are_outlined_and_meshes_survive_unrelated_pixel_edits() {
    let mut document = Document::new(32, 32);
    document.set_selection(Some(MaskCoverage::Dense(Arc::new(
        (0..1024)
            .map(|index| {
                let column = index % 32;
                let row = index / 32;
                if (4..28).contains(&column)
                    && (4..28).contains(&row)
                    && !((12..20).contains(&column) && (12..20).contains(&row))
                {
                    255
                } else {
                    0
                }
            })
            .collect(),
    ))));
    let mut view = View::default();
    view.zoom = 4.;
    let context = egui::Context::default();
    let mut cache = BoundaryCache::default();
    frame(&mut cache, &document, &view, &context);
    let rect = cache.key.unwrap().rect;
    let points: Vec<_> = cache.meshes[0]
        .vertices
        .iter()
        .map(|vertex| view.document(vertex.pos, rect, egui::vec2(32., 32.)))
        .collect();
    assert!(
        points
            .iter()
            .any(|point| (point.x - 12.).abs() < 0.2 && (12.2..19.8).contains(&point.y))
    );
    assert!(
        points
            .iter()
            .any(|point| (point.x - 4.).abs() < 0.2 && (4.2..27.8).contains(&point.y))
    );
    assert!(cache.meshes[0].vertices.len() <= MAX_QUADS * 4);
    let meshes = cache.meshes.clone();
    document.layers[1].opacity = 0.3;
    document.touch();
    frame(&mut cache, &document, &view, &context);
    assert_eq!(cache.rebuilds, 1);
    assert!(Arc::ptr_eq(&cache.meshes[0], &meshes[0]));
    assert!(Arc::ptr_eq(&cache.meshes[1], &meshes[1]));
    for (first, second) in cache.meshes[0]
        .vertices
        .iter()
        .zip(&cache.meshes[1].vertices)
    {
        assert_eq!(first.pos, second.pos);
        assert_ne!(first.color, second.color);
    }
}

#[test]
fn pan_rotate_flip_and_selection_replacement_invalidate_geometry_only() {
    let mut document = Document::new(64, 48);
    document.select_all_pixels();
    let context = egui::Context::default();
    let mut view = View::default();
    let mut cache = BoundaryCache::default();
    frame(&mut cache, &document, &view, &context);
    let original = cache.meshes[0].clone();
    view.pan = egui::vec2(20., 10.);
    frame(&mut cache, &document, &view, &context);
    assert_eq!(cache.rebuilds, 2);
    assert_eq!(original.vertices.len(), cache.meshes[0].vertices.len());
    for (before, after) in original.vertices.iter().zip(&cache.meshes[0].vertices) {
        assert!((before.pos + view.pan).distance(after.pos) < 0.001);
    }
    view.angle = 0.7;
    view.flip_horizontal = true;
    frame(&mut cache, &document, &view, &context);
    assert_eq!(cache.rebuilds, 3);
    assert_eq!(document.selection, Some(MaskCoverage::Uniform(255)));
    document.deselect_pixels();
    frame(&mut cache, &document, &view, &context);
    assert!(cache.meshes[0].vertices.is_empty());
    assert!(cache.source.is_none());
}

#[test]
fn checker_is_bounded_and_low_coverage_or_invalid_views_do_not_loop() {
    let mut document = Document::new(256, 256);
    document.set_selection(Some(MaskCoverage::Dense(Arc::new(
        (0..256 * 256)
            .map(|index| {
                if (index % 256 + index / 256) % 2 == 0 {
                    255
                } else {
                    0
                }
            })
            .collect(),
    ))));
    let context = egui::Context::default();
    let mut view = View::default();
    view.zoom = 2.;
    let mut cache = BoundaryCache::default();
    frame(&mut cache, &document, &view, &context);
    assert!(cache.step > 1);
    assert!(cache.meshes[0].vertices.capacity() <= MAX_QUADS * 4);
    assert!(cache.meshes[0].indices.capacity() <= MAX_QUADS * 6);
    document.set_selection(Some(MaskCoverage::Uniform(127)));
    frame(&mut cache, &document, &view, &context);
    assert!(cache.meshes[0].vertices.is_empty());
    assert!(cache.source.is_some());
    document.select_all_pixels();
    for zoom in [f32::NAN, 0., 1.0e-38, f32::INFINITY, 13.] {
        view.zoom = zoom;
        frame(&mut cache, &document, &view, &context);
        assert!(cache.meshes[0].vertices.is_empty());
    }
    view.zoom = 0.05;
    frame(&mut cache, &document, &view, &context);
    assert!(!cache.meshes[0].vertices.is_empty());
    view.zoom = 12.;
    frame(&mut cache, &document, &view, &context);
    assert!(cache.meshes[0].vertices.len() <= MAX_QUADS * 4);
}

#[test]
fn clipped_edges_use_document_coordinates_at_high_zoom_and_large_pan() {
    let mut document = Document::new(4096, 1);
    document.select_all_pixels();
    let context = egui::Context::default();
    let mut view = View::default();
    view.zoom = 12.;
    view.pan = egui::vec2(-20000., 0.);
    let mut cache = BoundaryCache::default();
    frame(&mut cache, &document, &view, &context);
    assert!(!cache.meshes[0].vertices.is_empty());
    let viewport = cache.key.unwrap().rect.expand(1.);
    assert!(
        cache
            .meshes
            .iter()
            .flat_map(|mesh| &mesh.vertices)
            .all(|vertex| { vertex.pos.is_finite() && viewport.contains(vertex.pos) })
    );
    view.pan.x = 100000.;
    frame(&mut cache, &document, &view, &context);
    assert!(cache.meshes[0].vertices.is_empty());
    document.width = u32::MAX;
    frame(&mut cache, &document, &view, &context);
    assert!(cache.meshes[0].vertices.is_empty());
}

#[test]
#[ignore]
fn selection_boundary_release_workload() {
    use std::{hint::black_box, time::Instant};
    for (size, checker) in [(512, false), (512, true), (2048, false), (2048, true)] {
        let mut document = Document::new(size, size);
        document.set_selection(Some(MaskCoverage::Dense(Arc::new(
            (0..size * size)
                .map(|index| {
                    let column = index % size;
                    let row = index / size;
                    if if checker {
                        (column + row) % 2 == 0
                    } else {
                        (size / 8..size * 7 / 8).contains(&column)
                            && (size / 8..size * 7 / 8).contains(&row)
                            && !((size * 3 / 8..size * 5 / 8).contains(&column)
                                && (size * 3 / 8..size * 5 / 8).contains(&row))
                    } {
                        255
                    } else {
                        0
                    }
                })
                .collect(),
        ))));
        let mut view = View::default();
        view.zoom = 512. / size as f32;
        let key = GeometryKey {
            rect: Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.)),
            dimensions: [size, size],
            zoom: view.zoom,
            angle: view.angle,
            pan: view.pan,
            flips: [false, false],
            pixels_per_point: 2.,
        };
        let original = document.clone();
        let mut cache = BoundaryCache::default();
        let mut samples = Vec::new();
        for _ in 0..21 {
            let start = Instant::now();
            cache.rebuild(black_box(&document), black_box(&view), key);
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
        samples.sort_by(f64::total_cmp);
        let bytes: usize = cache
            .meshes
            .iter()
            .map(|mesh| {
                mesh.vertices.capacity() * std::mem::size_of::<egui::epaint::Vertex>()
                    + mesh.indices.capacity() * std::mem::size_of::<u32>()
            })
            .sum();
        assert!(bytes <= MAX_QUADS * 208);
        assert!(document.same_edit_state(&original));
        for mesh in &cache.meshes {
            assert!(mesh.vertices.len() <= MAX_QUADS * 4);
            assert!(
                mesh.indices
                    .iter()
                    .all(|index| (*index as usize) < mesh.vertices.len())
            );
        }
        let start = Instant::now();
        for phase in 0..100000 {
            black_box(cache.meshes[phase & 1].clone());
        }
        eprintln!(
            "selection_boundary size={size} checker={checker} step={} median_ms={:.4} p95_ms={:.4} mesh_bytes={bytes} cached_phase_ns={:.2}",
            cache.step,
            samples[10],
            samples[19],
            start.elapsed().as_secs_f64() * 1e9 / 100000.
        );
    }
}
