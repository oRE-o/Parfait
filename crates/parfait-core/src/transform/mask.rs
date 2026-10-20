use super::*;
impl Document {
    pub fn mask_transform_bounds(&self, index: usize) -> Option<PixelRect> {
        let mask = self.layers.get(index)?.mask.as_ref()?;
        if self.selection.is_some() {
            self.selection_bounds()
        } else {
            crate::image_edit::bounds(self.width, self.height, |i| {
                mask.coverage.value(i) != 255
            })
        }
    }
    pub fn mask_transform_reason(
        &self,
        index: usize,
        t: &RasterTransform,
    ) -> Option<&'static str> {
        if !self.validate() || !t.validate() {
            return Some("마스크 또는 변형 값이 유효하지 않습니다.");
        }
        if !self.layer_ancestors_unlocked(index) {
            return Some("레이어와 부모의 잠금을 해제해 주세요.");
        }
        if self.layers.get(index).is_none_or(|l| l.mask.is_none()) {
            return Some("변형할 레이어 마스크가 없습니다.");
        }
        self.mask_transform_bounds(index)
            .is_none()
            .then_some("움직일 마스크 내용 또는 선택 범위가 없습니다.")
    }
    pub fn transform_layer_mask(
        &mut self,
        index: usize,
        t: &RasterTransform,
    ) -> bool {
        self.transform_layer_mask_cancellable(index, t, || false)
    }
    pub fn transform_layer_mask_cancellable(
        &mut self,
        index: usize,
        t: &RasterTransform,
        cancelled: impl Fn() -> bool,
    ) -> bool {
        if self.mask_transform_reason(index, t).is_some()
            || t.identity()
            || cancelled()
        {
            return false;
        }
        let target = &self.layers[index];
        let coverage = &target.mask.as_ref().unwrap().coverage;
        // Treat hidden coverage as alpha. The transparent exterior is the
        // mask's neutral 255, including during a selected-area transformation.
        let mut encoded =
            vec![0; self.width as usize * self.height as usize * 4];
        for (i, p) in encoded.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            *p = [255, 255, 255, 255 - coverage.value(i)];
        }
        let mut stage = self.clone();
        stage.contents.clear();
        stage.layers = vec![target.clone()];
        stage.active = 0;
        let layer = &mut stage.layers[0];
        layer.parent = None;
        layer.kind = LayerKind::Raster;
        layer.mask = None;
        layer.pixels = Arc::new(encoded);
        layer.visible = true;
        layer.locked = false;
        layer.alpha_locked = false;
        layer.clipped = false;
        layer.opacity = 1.;
        layer.blend_mode = crate::BlendMode::Normal;
        if !stage.transform_layer_pixels_cancellable(0, t, &cancelled)
            || cancelled()
        {
            return false;
        }
        let next = super::coverage(
            stage.layers[0]
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| 255 - p[3])
                .collect(),
        );
        if next == *coverage && self.selection == stage.selection {
            return false;
        }
        self.layers[index].mask.as_mut().unwrap().coverage = next;
        self.selection = stage.selection;
        self.touch();
        true
    }
}
