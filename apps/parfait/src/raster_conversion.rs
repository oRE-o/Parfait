use crate::{app::Parfait, shortcuts::Command};
use parfait_core::RasterConversion;

impl Parfait {
    pub(crate) fn raster_conversion_disabled_reason(
        &self,
        conversion: RasterConversion,
    ) -> Option<&'static str> {
        if self.dialog_open()
            || self.documents.active_session.layer_ui.list.owns_pointer()
            || self.workspace.dragging.is_some()
            || self.workspace.resizing
        {
            return Some(
                "Finish the current gesture or dialog before converting pixels.",
            );
        }
        if self.editing_layer_mask() {
            return Some(
                "Select the layer's content thumbnail, not its mask, to convert pixels.",
            );
        }
        self.documents.active_session.document.raster_conversion_reason(
            self.documents.active_session.document.active,
            conversion,
        )
    }

    pub(crate) fn execute_raster_conversion(&mut self, command: Command) {
        let Some(conversion) = command.raster_conversion(self.brush.color)
        else {
            return;
        };
        if let Some(reason) = self.command_disabled_reason(command) {
            self.notice = Some(reason.into());
            return;
        }
        let layer = self.documents.active_session.document.active;
        self.edit_named(command.label(), |document| {
            document.convert_layer_pixels(layer, conversion);
        });
    }
}

#[cfg(test)]
mod tests;
