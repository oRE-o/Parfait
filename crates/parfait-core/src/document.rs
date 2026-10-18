use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,
    pub pixels: Arc<Vec<u8>>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Document {
    pub width: u32,
    pub height: u32,
    pub name: String,
    /// Bottom to top. Selection is an index into this list.
    pub layers: Vec<Layer>,
    pub active: usize,
    pub revision: u64,
}

impl Document {
    pub fn new(width: u32, height: u32) -> Self {
        assert!(width > 0 && height > 0 && width <= 4096 && height <= 4096);
        let len = width as usize * height as usize * 4;
        Self {
            width,
            height,
            name: "Untitled".into(),
            layers: vec![
                Layer {
                    name: "Paper".into(),
                    visible: true,
                    locked: true,
                    opacity: 1.,
                    pixels: Arc::new(vec![255; len]),
                },
                Layer {
                    name: "Layer 1".into(),
                    visible: true,
                    locked: false,
                    opacity: 1.,
                    pixels: Arc::new(vec![0; len]),
                },
            ],
            active: 1,
            revision: 1,
        }
    }
    pub fn validate(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.width <= 4096
            && self.height <= 4096
            && !self.layers.is_empty()
            && self.layers.len() <= 64
            && self.active < self.layers.len()
            && self.layers.iter().all(|l| {
                l.pixels.len() == self.width as usize * self.height as usize * 4
                    && l.opacity.is_finite()
                    && (0.0..=1.0).contains(&l.opacity)
            })
    }
    pub fn touch(&mut self) {
        self.revision += 1;
    }
    pub fn add_layer(&mut self) {
        if self.layers.len() >= 64 {
            return;
        }
        let layer = Layer {
            name: format!("Layer {}", self.layers.len()),
            visible: true,
            locked: false,
            opacity: 1.,
            pixels: Arc::new(vec![0; self.width as usize * self.height as usize * 4]),
        };
        self.active += 1;
        self.layers.insert(self.active, layer);
        self.touch();
    }
    pub fn composite(&self) -> Vec<u8> {
        let mut out = vec![0; self.width as usize * self.height as usize * 4];
        for layer in self.layers.iter().filter(|l| l.visible && l.opacity > 0.) {
            for (dst, src) in out
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(layer.pixels.as_chunks::<4>().0.iter())
            {
                composite_over(
                    dst,
                    [src[0], src[1], src[2]],
                    src[3] as f32 / 255. * layer.opacity,
                );
            }
        }
        out
    }
}

pub fn composite_over(dst: &mut [u8], rgb: [u8; 3], alpha: f32) {
    let sa = alpha.clamp(0., 1.);
    if sa == 0. {
        return;
    }
    let da = dst[3] as f32 / 255.;
    let a = sa + da * (1. - sa);
    for i in 0..3 {
        dst[i] = ((rgb[i] as f32 * sa + dst[i] as f32 * da * (1. - sa)) / a).round() as u8;
    }
    dst[3] = (a * 255.).round() as u8;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layer_order_opacity_and_visibility() {
        let mut d = Document::new(1, 1);
        d.layers[1].pixels = Arc::new(vec![255, 0, 0, 255]);
        d.layers[1].opacity = 0.5;
        assert_eq!(d.composite(), vec![255, 128, 128, 255]);
        d.layers[1].visible = false;
        assert_eq!(d.composite(), vec![255; 4]);
    }
    #[test]
    fn snapshot_pixels_are_copy_on_write() {
        let mut d = Document::new(2, 2);
        let before = d.clone();
        Arc::make_mut(&mut d.layers[1].pixels)[0] = 123;
        assert_eq!(before.layers[1].pixels[0], 0);
        assert_eq!(d.layers[1].pixels[0], 123);
    }
}
