use crate::Document;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterConversion {
    DrawingColor([u8; 3]),
    BrightnessToOpacity,
}

impl RasterConversion {
    fn pixel(self, pixel: [u8; 4]) -> [u8; 4] {
        if pixel[3] == 0 {
            return pixel;
        }
        match self {
            Self::DrawingColor(color) => {
                [color[0], color[1], color[2], pixel[3]]
            }
            Self::BrightnessToOpacity => {
                let brightness = (u32::from(pixel[0]) * 299
                    + u32::from(pixel[1]) * 587
                    + u32::from(pixel[2]) * 114
                    + 500)
                    / 1000;
                let alpha =
                    (u32::from(pixel[3]) * (255 - brightness) + 127) / 255;
                [0, 0, 0, alpha as u8]
            }
        }
    }
}

impl Document {
    pub fn raster_conversion_reason(
        &self,
        layer: usize,
        conversion: RasterConversion,
    ) -> Option<&'static str> {
        if !self.validate() {
            return Some("The drawing has an invalid layer layout.");
        }
        let Some(target) = self.layers.get(layer) else {
            return Some("The target layer no longer exists.");
        };
        if target.kind != crate::LayerKind::Raster {
            return Some("Select a raster layer before converting its pixels.");
        }
        if !self.layer_ancestors_unlocked(layer) {
            return Some("Unlock the layer before converting its pixels.");
        }
        if conversion == RasterConversion::BrightnessToOpacity
            && target.alpha_locked
        {
            return Some(
                "Unlock transparent pixels before converting brightness to opacity.",
            );
        }
        None
    }

    pub fn convert_layer_pixels(
        &mut self,
        layer: usize,
        conversion: RasterConversion,
    ) -> bool {
        if self.raster_conversion_reason(layer, conversion).is_some() {
            return false;
        }
        let source = &self.layers[layer].pixels;
        let convert = |index: usize, pixel: [u8; 4]| {
            let converted = conversion.pixel(pixel);
            match &self.selection {
                None => converted,
                Some(coverage) => crate::selection::interpolate_pixel(
                    pixel,
                    converted,
                    coverage.value(index),
                ),
            }
        };
        let Some(first) = source
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .position(|(index, &pixel)| convert(index, pixel) != pixel)
        else {
            return false;
        };
        let selection = self.selection.as_ref();
        for (offset, pixel) in Arc::make_mut(&mut self.layers[layer].pixels)
            .as_chunks_mut::<4>()
            .0[first..]
            .iter_mut()
            .enumerate()
        {
            *pixel = match selection {
                None => conversion.pixel(*pixel),
                Some(coverage) => crate::selection::interpolate_pixel(
                    *pixel,
                    conversion.pixel(*pixel),
                    coverage.value(first + offset),
                ),
            };
        }
        self.touch();
        true
    }
}

#[cfg(test)]
mod tests;
