use crate::{app::Parfait, brushes::BrushGroup, display::ViewCommand, widgets, workspace::Palette};
use eframe::egui;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Command {
    NewDrawing,
    OpenProject,
    SaveProject,
    SaveAs,
    ExportPng,
    Undo,
    Redo,
    NewLayer,
    DuplicateLayer,
    ZoomIn,
    ZoomOut,
    ActualSize,
    DoubleSize,
    FitCanvas,
    ResetDisplay,
    RotateLeft,
    RotateRight,
    ResetRotation,
    ResetOrientation,
    FlipHorizontal,
    FlipVertical,
    RulerBar,
    PixelGrid,
    DisplaySettings,
    ShortcutSettings,
    HidePalettes,
    ResetPaletteLayout,
    PaletteBrushes,
    PaletteProperties,
    PaletteSizes,
    PaletteColor,
    PaletteNavigator,
    PaletteLayers,
    PaletteHistory,
    ToolInk,
    ToolSketch,
    ToolPaint,
    ToolAirbrush,
    ToolEraser,
    ToolBlend,
    ToolHand,
    ToolPicker,
    BrushSmaller,
    BrushLarger,
    SubToolDetail,
    ShortcutGuide,
    Notices,
    DeleteLayer,
    MoveLayerUp,
    MoveLayerDown,
    LayerVisibility,
    LayerLock,
    TransferLayerDown,
    MergeLayerDown,
    MergeSelectedLayers,
    MergeVisibleLayers,
    CopyVisibleLayers,
    FlattenImage,
    ClearLayer,
    LayerAlphaLock,
    LayerClipping,
    RenameLayer,
    MoveActiveLayerUp,
    MoveActiveLayerDown,
    MoveActiveLayerToTop,
    MoveActiveLayerToBottom,
    SelectLayerAbove,
    SelectLayerBelow,
    LayerMaskRevealAll,
    LayerMaskHideAll,
    DeleteLayerMask,
    ApplyLayerMask,
    LayerMaskEnabled,
    ClearEditingTarget,
    ClearLayerMask,
    ShowMaskArea,
    ConvertToDrawingColor,
    BrightnessToOpacity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    File,
    Edit,
    Layer,
    View,
    Window,
    Tools,
    Options,
    Help,
}

impl Category {
    pub const ALL: &[Self] = &[
        Self::File,
        Self::Edit,
        Self::Layer,
        Self::View,
        Self::Window,
        Self::Tools,
        Self::Options,
        Self::Help,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Edit => "Edit",
            Self::Layer => "Layer",
            Self::View => "View",
            Self::Window => "Window",
            Self::Tools => "Tools",
            Self::Options => "Options",
            Self::Help => "Help",
        }
    }
}

impl Command {
    pub const ALL: &[Self] = &[
        Self::NewDrawing,
        Self::OpenProject,
        Self::SaveProject,
        Self::SaveAs,
        Self::ExportPng,
        Self::Undo,
        Self::Redo,
        Self::NewLayer,
        Self::DuplicateLayer,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::ActualSize,
        Self::DoubleSize,
        Self::FitCanvas,
        Self::ResetDisplay,
        Self::RotateLeft,
        Self::RotateRight,
        Self::ResetRotation,
        Self::ResetOrientation,
        Self::FlipHorizontal,
        Self::FlipVertical,
        Self::RulerBar,
        Self::PixelGrid,
        Self::DisplaySettings,
        Self::ShortcutSettings,
        Self::HidePalettes,
        Self::ResetPaletteLayout,
        Self::PaletteBrushes,
        Self::PaletteProperties,
        Self::PaletteSizes,
        Self::PaletteColor,
        Self::PaletteNavigator,
        Self::PaletteLayers,
        Self::PaletteHistory,
        Self::ToolInk,
        Self::ToolSketch,
        Self::ToolPaint,
        Self::ToolAirbrush,
        Self::ToolEraser,
        Self::ToolBlend,
        Self::ToolHand,
        Self::ToolPicker,
        Self::BrushSmaller,
        Self::BrushLarger,
        Self::SubToolDetail,
        Self::ShortcutGuide,
        Self::Notices,
        Self::DeleteLayer,
        Self::MoveLayerUp,
        Self::MoveLayerDown,
        Self::LayerVisibility,
        Self::LayerLock,
        Self::TransferLayerDown,
        Self::MergeLayerDown,
        Self::MergeSelectedLayers,
        Self::MergeVisibleLayers,
        Self::CopyVisibleLayers,
        Self::FlattenImage,
        Self::ClearLayer,
        Self::LayerAlphaLock,
        Self::LayerClipping,
        Self::RenameLayer,
        Self::MoveActiveLayerUp,
        Self::MoveActiveLayerDown,
        Self::MoveActiveLayerToTop,
        Self::MoveActiveLayerToBottom,
        Self::SelectLayerAbove,
        Self::SelectLayerBelow,
        Self::LayerMaskRevealAll,
        Self::LayerMaskHideAll,
        Self::DeleteLayerMask,
        Self::ApplyLayerMask,
        Self::LayerMaskEnabled,
        Self::ClearEditingTarget,
        Self::ClearLayerMask,
        Self::ShowMaskArea,
        Self::ConvertToDrawingColor,
        Self::BrightnessToOpacity,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::NewDrawing => "New drawing",
            Self::OpenProject => "Open project",
            Self::SaveProject => "Save project",
            Self::SaveAs => "Save as",
            Self::ExportPng => "Export PNG",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::NewLayer => "New raster layer",
            Self::DuplicateLayer => "Duplicate layer",
            Self::ZoomIn => "Zoom in",
            Self::ZoomOut => "Zoom out",
            Self::ActualSize => "100%",
            Self::DoubleSize => "200%",
            Self::FitCanvas => "Fit canvas",
            Self::ResetDisplay => "Reset display",
            Self::RotateLeft => "Rotate left",
            Self::RotateRight => "Rotate right",
            Self::ResetRotation => "Reset rotation",
            Self::ResetOrientation => "Reset rotation / flip",
            Self::FlipHorizontal => "Flip horizontal",
            Self::FlipVertical => "Flip vertical",
            Self::RulerBar => "Ruler bar",
            Self::PixelGrid => "Pixel grid (8×+)",
            Self::DisplaySettings => "Display settings…",
            Self::ShortcutSettings => "Shortcut settings…",
            Self::HidePalettes => "Hide palettes",
            Self::ResetPaletteLayout => "Reset palette layout",
            Self::PaletteBrushes => "Sub tools palette",
            Self::PaletteProperties => "Tool property palette",
            Self::PaletteSizes => "Brush size palette",
            Self::PaletteColor => "Color wheel palette",
            Self::PaletteNavigator => "Navigator palette",
            Self::PaletteLayers => "Layers palette",
            Self::PaletteHistory => "History palette",
            Self::ToolInk => "Ink / pen",
            Self::ToolSketch => "Sketch / pencil",
            Self::ToolPaint => "Paint / marker",
            Self::ToolAirbrush => "Airbrush",
            Self::ToolEraser => "Eraser",
            Self::ToolBlend => "Blend / blur / smudge",
            Self::ToolHand => "Hand",
            Self::ToolPicker => "Eyedropper",
            Self::BrushSmaller => "Smaller brush",
            Self::BrushLarger => "Larger brush",
            Self::SubToolDetail => "Sub tool detail",
            Self::ShortcutGuide => "Keyboard shortcuts",
            Self::Notices => "Third-party notices",
            Self::DeleteLayer => "Delete layer",
            Self::MoveLayerUp => "Move selected layers up",
            Self::MoveLayerDown => "Move selected layers down",
            Self::LayerVisibility => "Layer visible",
            Self::LayerLock => "Layer locked",
            Self::TransferLayerDown => "Transfer to layer below",
            Self::MergeLayerDown => "Merge with layer below",
            Self::MergeSelectedLayers => "Merge selected layers",
            Self::MergeVisibleLayers => "Merge visible layers",
            Self::CopyVisibleLayers => "Merge visible to new layer",
            Self::FlattenImage => "Flatten image",
            Self::ClearLayer => "Clear selected layers",
            Self::LayerAlphaLock => "Lock transparent pixels",
            Self::LayerClipping => "Clip to layer below",
            Self::RenameLayer => "Rename layer…",
            Self::MoveActiveLayerUp => "Bring active layer forward",
            Self::MoveActiveLayerDown => "Send active layer backward",
            Self::MoveActiveLayerToTop => "Bring active layer to front",
            Self::MoveActiveLayerToBottom => "Send active layer to back",
            Self::SelectLayerAbove => "Select layer above",
            Self::SelectLayerBelow => "Select layer below",
            Self::LayerMaskRevealAll => "Create mask: reveal all",
            Self::LayerMaskHideAll => "Create mask: hide all",
            Self::DeleteLayerMask => "Delete layer mask",
            Self::ApplyLayerMask => "Apply mask to layer",
            Self::LayerMaskEnabled => "Layer mask enabled",
            Self::ClearEditingTarget => "Clear editing target",
            Self::ClearLayerMask => "Clear layer mask",
            Self::ShowMaskArea => "Show mask area",
            Self::ConvertToDrawingColor => "Convert to drawing color",
            Self::BrightnessToOpacity => "Convert brightness to opacity",
        }
    }

    pub fn category(self) -> Category {
        match self {
            Self::NewDrawing
            | Self::OpenProject
            | Self::SaveProject
            | Self::SaveAs
            | Self::ExportPng => Category::File,
            Self::Undo
            | Self::Redo
            | Self::ShortcutSettings
            | Self::ClearEditingTarget
            | Self::ConvertToDrawingColor
            | Self::BrightnessToOpacity => Category::Edit,
            Self::NewLayer
            | Self::DuplicateLayer
            | Self::DeleteLayer
            | Self::MoveLayerUp
            | Self::MoveLayerDown
            | Self::LayerVisibility
            | Self::LayerLock
            | Self::TransferLayerDown
            | Self::MergeLayerDown
            | Self::MergeSelectedLayers
            | Self::MergeVisibleLayers
            | Self::CopyVisibleLayers
            | Self::FlattenImage
            | Self::ClearLayer
            | Self::LayerAlphaLock
            | Self::LayerClipping
            | Self::RenameLayer
            | Self::MoveActiveLayerUp
            | Self::MoveActiveLayerDown
            | Self::MoveActiveLayerToTop
            | Self::MoveActiveLayerToBottom
            | Self::SelectLayerAbove
            | Self::SelectLayerBelow
            | Self::LayerMaskRevealAll
            | Self::LayerMaskHideAll
            | Self::DeleteLayerMask
            | Self::ApplyLayerMask
            | Self::LayerMaskEnabled
            | Self::ClearLayerMask
            | Self::ShowMaskArea => Category::Layer,
            Self::HidePalettes
            | Self::ResetPaletteLayout
            | Self::PaletteBrushes
            | Self::PaletteProperties
            | Self::PaletteSizes
            | Self::PaletteColor
            | Self::PaletteNavigator
            | Self::PaletteLayers
            | Self::PaletteHistory => Category::Window,
            Self::ToolInk
            | Self::ToolSketch
            | Self::ToolPaint
            | Self::ToolAirbrush
            | Self::ToolEraser
            | Self::ToolBlend
            | Self::ToolHand
            | Self::ToolPicker => Category::Tools,
            Self::BrushSmaller | Self::BrushLarger | Self::SubToolDetail => Category::Options,
            Self::ShortcutGuide | Self::Notices => Category::Help,
            _ => Category::View,
        }
    }

    pub fn is_tool(self) -> bool {
        self.category() == Category::Tools
    }
    pub(crate) fn raster_conversion(
        self,
        color: [u8; 3],
    ) -> Option<parfait_core::RasterConversion> {
        Some(match self {
            Self::ConvertToDrawingColor => parfait_core::RasterConversion::DrawingColor(color),
            Self::BrightnessToOpacity => parfait_core::RasterConversion::BrightnessToOpacity,
            _ => return None,
        })
    }
    pub(crate) fn is_layer_mask(self) -> bool {
        matches!(
            self,
            Self::LayerMaskRevealAll
                | Self::LayerMaskHideAll
                | Self::DeleteLayerMask
                | Self::ApplyLayerMask
                | Self::LayerMaskEnabled
                | Self::ClearLayerMask
                | Self::ShowMaskArea
        )
    }
    pub fn layer_combine(self) -> Option<parfait_core::LayerCombine> {
        use parfait_core::LayerCombine;
        Some(match self {
            Self::TransferLayerDown => LayerCombine::TransferDown,
            Self::MergeLayerDown => LayerCombine::MergeDown,
            Self::MergeSelectedLayers => LayerCombine::MergeSelected,
            Self::MergeVisibleLayers => LayerCombine::MergeVisible,
            Self::CopyVisibleLayers => LayerCombine::CopyVisible,
            Self::FlattenImage => LayerCombine::Flatten,
            Self::ClearLayer => LayerCombine::Clear,
            _ => return None,
        })
    }
    pub fn allows_modifier_fallback(self) -> bool {
        self.is_tool()
            || matches!(
                self,
                Self::BrushSmaller | Self::BrushLarger | Self::HidePalettes | Self::ShortcutGuide
            )
    }
    pub fn repeatable(self) -> bool {
        matches!(
            self,
            Self::Undo
                | Self::Redo
                | Self::ZoomIn
                | Self::ZoomOut
                | Self::RotateLeft
                | Self::RotateRight
                | Self::BrushSmaller
                | Self::BrushLarger
                | Self::SelectLayerAbove
                | Self::SelectLayerBelow
        )
    }
    pub fn palette(self) -> Option<Palette> {
        Some(match self {
            Self::PaletteBrushes => Palette::Brushes,
            Self::PaletteProperties => Palette::Properties,
            Self::PaletteSizes => Palette::Sizes,
            Self::PaletteColor => Palette::Color,
            Self::PaletteNavigator => Palette::Navigator,
            Self::PaletteLayers => Palette::Layers,
            Self::PaletteHistory => Palette::History,
            _ => return None,
        })
    }
}

impl Parfait {
    pub fn execute_command(&mut self, command: Command, ctx: &egui::Context) {
        if let Some(reason) = self.command_disabled_reason(command) {
            self.notice = Some(reason.into());
            return;
        }
        match command {
            Command::NewDrawing => self.confirm_new = true,
            Command::OpenProject => self.open_project(),
            Command::SaveProject => self.save_project(false),
            Command::SaveAs => self.save_project(true),
            Command::ExportPng => self.export(),
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::ConvertToDrawingColor | Command::BrightnessToOpacity => {
                self.execute_raster_conversion(command);
            }
            Command::NewLayer => self.new_layer(),
            Command::ClearEditingTarget => {
                self.execute_command(
                    if self.editing_layer_mask() {
                        Command::ClearLayerMask
                    } else {
                        Command::ClearLayer
                    },
                    ctx,
                );
            }
            Command::LayerMaskRevealAll
            | Command::LayerMaskHideAll
            | Command::DeleteLayerMask
            | Command::ApplyLayerMask
            | Command::LayerMaskEnabled
            | Command::ClearLayerMask
            | Command::ShowMaskArea => self.execute_mask_command(command),
            Command::DuplicateLayer => self.duplicate_selected_layers(),
            Command::DeleteLayer
            | Command::MoveLayerUp
            | Command::MoveLayerDown
            | Command::LayerVisibility
            | Command::LayerLock
            | Command::LayerAlphaLock
            | Command::LayerClipping => self.execute_selected_layer_command(command),
            Command::RenameLayer
            | Command::MoveActiveLayerUp
            | Command::MoveActiveLayerDown
            | Command::MoveActiveLayerToTop
            | Command::MoveActiveLayerToBottom
            | Command::SelectLayerAbove
            | Command::SelectLayerBelow => self.execute_layer_organization(command),
            Command::TransferLayerDown
            | Command::MergeLayerDown
            | Command::MergeSelectedLayers
            | Command::MergeVisibleLayers
            | Command::CopyVisibleLayers
            | Command::FlattenImage
            | Command::ClearLayer => self.combine_layer_command(command),
            Command::ZoomIn => self.view.step_zoom(1),
            Command::ZoomOut => self.view.step_zoom(-1),
            Command::ActualSize => self.view_command(ViewCommand::ActualSize),
            Command::DoubleSize => self.view_command(ViewCommand::DoubleSize),
            Command::FitCanvas => self.view_command(ViewCommand::Fit),
            Command::ResetDisplay => self.view_command(ViewCommand::Reset),
            Command::RotateLeft => self.view_command(ViewCommand::RotateLeft),
            Command::RotateRight => self.view_command(ViewCommand::RotateRight),
            Command::ResetRotation => self.view_command(ViewCommand::ResetRotation),
            Command::ResetOrientation => self.view_command(ViewCommand::ResetOrientation),
            Command::FlipHorizontal => self.view_command(ViewCommand::FlipHorizontal),
            Command::FlipVertical => self.view_command(ViewCommand::FlipVertical),
            Command::RulerBar => self.display.rulers = !self.display.rulers,
            Command::PixelGrid => self.display.pixel_grid = !self.display.pixel_grid,
            Command::DisplaySettings => self.show_display_settings = true,
            Command::ShortcutSettings => self.open_shortcut_settings(),
            Command::HidePalettes => self.hide_panels = !self.hide_panels,
            Command::ResetPaletteLayout => {
                self.workspace = Default::default();
                ctx.data_mut(|data| {
                    for dock in ["dock_left", "dock_right"] {
                        data.remove::<egui::containers::panel::PanelState>(egui::Id::new(dock));
                    }
                });
            }
            Command::ToolInk => self.select_brush_group(BrushGroup::Ink),
            Command::ToolSketch => self.select_brush_group(BrushGroup::Sketch),
            Command::ToolPaint => self.select_brush_group(BrushGroup::Paint),
            Command::ToolAirbrush => self.select_brush_group(BrushGroup::Airbrush),
            Command::ToolEraser => self.select_brush_group(BrushGroup::Eraser),
            Command::ToolBlend => self.select_brush_group(BrushGroup::Blend),
            Command::ToolHand => self.navigation_tool = 1,
            Command::ToolPicker => self.navigation_tool = 2,
            Command::BrushSmaller => self.brush.size = (self.brush.size / 1.2).max(1.),
            Command::BrushLarger => self.brush.size = (self.brush.size * 1.2).min(512.),
            Command::SubToolDetail => self.brush_details.open = true,
            Command::ShortcutGuide => self.show_help = true,
            Command::Notices => self.show_licenses = true,
            _ => {
                if let Some(palette) = command.palette()
                    && let Some(placement) = self
                        .workspace
                        .panels
                        .iter_mut()
                        .find(|placement| placement.palette == palette)
                {
                    placement.visible = !placement.visible;
                }
            }
        }
    }

    pub fn active_tool_command(&self) -> Option<Command> {
        Some(match self.navigation_tool {
            1 => Command::ToolHand,
            2 => Command::ToolPicker,
            _ => match BrushGroup::of(&self.brush) {
                BrushGroup::Ink => Command::ToolInk,
                BrushGroup::Sketch => Command::ToolSketch,
                BrushGroup::Paint => Command::ToolPaint,
                BrushGroup::Airbrush => Command::ToolAirbrush,
                BrushGroup::Eraser => Command::ToolEraser,
                BrushGroup::Blend => Command::ToolBlend,
                BrushGroup::Materials => return None,
            },
        })
    }

    pub fn command_menu_item(&mut self, ui: &mut egui::Ui, command: Command) {
        self.command_menu_item_named(ui, command, command.label());
    }

    pub(crate) fn command_menu_item_named(
        &mut self,
        ui: &mut egui::Ui,
        command: Command,
        label: &str,
    ) {
        let reason = self.command_disabled_reason(command);
        let response = ui
            .add_enabled_ui(reason.is_none(), |ui| {
                widgets::menu_item(ui, label, self.keymap.hint(command))
            })
            .inner;
        #[cfg(test)]
        ui.data_mut(|data| {
            data.insert_temp(egui::Id::new(("menu_item", command.label())), response.rect)
        });
        if response.clicked() {
            self.execute_command(command, ui.ctx());
            ui.close();
        }
        if let Some(reason) = reason {
            response.on_disabled_hover_text(reason);
        } else {
            response.on_hover_text(self.keymap.tooltip(command));
        }
    }

    pub fn command_toggle(&mut self, ui: &mut egui::Ui, command: Command, selected: bool) {
        self.command_toggle_named(ui, command, selected, command.label());
    }

    pub(crate) fn command_toggle_named(
        &mut self,
        ui: &mut egui::Ui,
        command: Command,
        selected: bool,
        label: &str,
    ) {
        let reason = self.command_disabled_reason(command);
        let response = ui
            .add_enabled_ui(reason.is_none(), |ui| {
                widgets::menu_toggle(ui, label, self.keymap.hint(command), selected)
            })
            .inner;
        #[cfg(test)]
        ui.data_mut(|data| {
            data.insert_temp(egui::Id::new(("command_toggle", command)), response.rect)
        });
        if response.clicked() {
            self.execute_command(command, ui.ctx());
        }
        if let Some(reason) = reason {
            response.on_disabled_hover_text(reason);
        } else {
            response.on_hover_text(self.keymap.tooltip(command));
        }
    }

    pub fn command_icon(&mut self, ui: &mut egui::Ui, icon: widgets::Icon, command: Command) {
        let reason = self.command_disabled_reason(command);
        let response = ui
            .add_enabled_ui(reason.is_none(), |ui| {
                widgets::icon(ui, icon, false, self.keymap.tooltip(command))
            })
            .inner;
        if response.clicked() {
            self.execute_command(command, ui.ctx());
        }
        if let Some(reason) = reason {
            response.on_disabled_hover_text(reason);
        }
    }
}
