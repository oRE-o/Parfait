use crate::{
    app::Parfait, brushes::BrushGroup, display::ViewCommand, widgets,
    workspace::Palette,
};
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
    LayerReference,
    LayerDraft,
    LayerDetails,
    NewLayerFolder,
    GroupLayersInFolder,
    UngroupLayerFolder,
    LayerSelectionCreate,
    LayerSelectionAdd,
    LayerSelectionSubtract,
    LayerSelectionIntersect,
    LayerMaskOutsideSelection,
    LayerMaskSelection,
    SelectAll,
    Deselect,
    InvertSelection,
    EditCut,
    EditCopy,
    EditPaste,
    EditPasteAtView,
    EditClearOutside,
    EditFill,
    EditAdvancedFill,
    EditBorder,
    EditBrightnessContrast,
    EditHsl,
    EditPosterize,
    EditInvert,
    EditLevels,
    EditToneCurve,
    EditColorBalance,
    EditThreshold,
    EditGradientMap,
    EditResolution,
    EditCanvasSize,
    EditCrop,
    EditCanvasRotateRight,
    EditCanvasRotateHalf,
    EditCanvasRotateLeft,
    EditCanvasFlipX,
    EditCanvasFlipY,
    EditClearHistory,
    EditPressure,
    EditAlignTop,
    EditAlignCenterY,
    EditAlignBottom,
    EditAlignLeft,
    EditAlignCenterX,
    EditAlignRight,
    EditDistributeTop,
    EditDistributeCenterY,
    EditDistributeBottom,
    EditDistributeLeft,
    EditDistributeCenterX,
    EditDistributeRight,
    EditSpaceY,
    EditSpaceX,
    EditRefBounds,
    EditRefCanvas,
    EditRefSelection,
    EditRefGuides,
    EditRefAuto,
    EditTransformScaleRotate,
    EditTransformScale,
    EditTransformRotate,
    EditTransformFree,
    EditTransformDistort,
    EditTransformShear,
    EditTransformPerspective,
    EditTransformFlipX,
    EditTransformFlipY,
    EditTransformMesh,
    EditTransformPins,
    EditTransformApply,
    EditTransformCancel,
    EditSuperResolution,
    EditAutoColor,
    EditAutoColorHints,
    EditAutoColorOptions,
    EditAutoShade,
    EditColorReference,
    EditMaterialImage,
    EditMaterialTemplate,
    EditPreferences,
    EditModifierSettings,
    EditCommandBarSettings,
    EditFontSettings,
    EditControllerSettings,
    EditQuickSettings,
    EditToolCycleSettings,
    EditWirelessSearch,
    EditControllerAdd,
    EditExternalDevice,
    EditPrivacySettings,
    EditSystemServices,
    EditCanvasBasic,
    EditCopySvg,
    ShowQuickMenu,
    CycleTools,
    EditScreenColor,
    EditScreenColorHide,
    EditTextInkBounds,
    EditVectorCenters,
    NewTextLayer,
    NewVectorLayer,
    EditNativeLayer,
    RasterizeNativeLayer,
    NextCanvas,
    PreviousCanvas,
    CloseCanvas,
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
        Self::LayerReference,
        Self::LayerDraft,
        Self::LayerDetails,
        Self::NewLayerFolder,
        Self::GroupLayersInFolder,
        Self::UngroupLayerFolder,
        Self::LayerSelectionCreate,
        Self::LayerSelectionAdd,
        Self::LayerSelectionSubtract,
        Self::LayerSelectionIntersect,
        Self::LayerMaskOutsideSelection,
        Self::LayerMaskSelection,
        Self::SelectAll,
        Self::Deselect,
        Self::InvertSelection,
        Self::EditCut,
        Self::EditCopy,
        Self::EditPaste,
        Self::EditPasteAtView,
        Self::EditClearOutside,
        Self::EditFill,
        Self::EditAdvancedFill,
        Self::EditBorder,
        Self::EditBrightnessContrast,
        Self::EditHsl,
        Self::EditPosterize,
        Self::EditInvert,
        Self::EditLevels,
        Self::EditToneCurve,
        Self::EditColorBalance,
        Self::EditThreshold,
        Self::EditGradientMap,
        Self::EditResolution,
        Self::EditCanvasSize,
        Self::EditCrop,
        Self::EditCanvasRotateRight,
        Self::EditCanvasRotateHalf,
        Self::EditCanvasRotateLeft,
        Self::EditCanvasFlipX,
        Self::EditCanvasFlipY,
        Self::EditClearHistory,
        Self::EditPressure,
        Self::EditAlignTop,
        Self::EditAlignCenterY,
        Self::EditAlignBottom,
        Self::EditAlignLeft,
        Self::EditAlignCenterX,
        Self::EditAlignRight,
        Self::EditDistributeTop,
        Self::EditDistributeCenterY,
        Self::EditDistributeBottom,
        Self::EditDistributeLeft,
        Self::EditDistributeCenterX,
        Self::EditDistributeRight,
        Self::EditSpaceY,
        Self::EditSpaceX,
        Self::EditRefBounds,
        Self::EditRefCanvas,
        Self::EditRefSelection,
        Self::EditRefGuides,
        Self::EditRefAuto,
        Self::EditTransformScaleRotate,
        Self::EditTransformScale,
        Self::EditTransformRotate,
        Self::EditTransformFree,
        Self::EditTransformDistort,
        Self::EditTransformShear,
        Self::EditTransformPerspective,
        Self::EditTransformFlipX,
        Self::EditTransformFlipY,
        Self::EditTransformMesh,
        Self::EditTransformPins,
        Self::EditTransformApply,
        Self::EditTransformCancel,
        Self::EditSuperResolution,
        Self::EditAutoColor,
        Self::EditAutoColorHints,
        Self::EditAutoColorOptions,
        Self::EditAutoShade,
        Self::EditColorReference,
        Self::EditMaterialImage,
        Self::EditMaterialTemplate,
        Self::EditPreferences,
        Self::EditModifierSettings,
        Self::EditCommandBarSettings,
        Self::EditFontSettings,
        Self::EditControllerSettings,
        Self::EditQuickSettings,
        Self::EditToolCycleSettings,
        Self::EditWirelessSearch,
        Self::EditControllerAdd,
        Self::EditExternalDevice,
        Self::EditPrivacySettings,
        Self::EditSystemServices,
        Self::EditCanvasBasic,
        Self::EditCopySvg,
        Self::ShowQuickMenu,
        Self::CycleTools,
        Self::EditScreenColor,
        Self::EditScreenColorHide,
        Self::EditTextInkBounds,
        Self::EditVectorCenters,
        Self::NewTextLayer,
        Self::NewVectorLayer,
        Self::EditNativeLayer,
        Self::RasterizeNativeLayer,
        Self::NextCanvas,
        Self::PreviousCanvas,
        Self::CloseCanvas,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::NextCanvas => "다음 캔버스",
            Self::PreviousCanvas => "이전 캔버스",
            Self::CloseCanvas => "닫기",
            Self::EditTextInkBounds => "텍스트 그리기 경계 정렬",
            Self::EditVectorCenters => "벡터 중심선 정렬",
            Self::NewTextLayer => "텍스트 레이어…",
            Self::NewVectorLayer => "벡터 레이어…",
            Self::EditNativeLayer => "텍스트 / 벡터 내용 편집…",
            Self::RasterizeNativeLayer => "텍스트 / 벡터 래스터화",
            Self::EditScreenColor => "화면색 추출…",
            Self::EditScreenColorHide => "창을 숨기고 화면색 추출…",
            Self::EditPreferences => "환경설정…",
            Self::EditModifierSettings => "보조키 설정…",
            Self::EditCommandBarSettings => "커맨드 바 설정…",
            Self::EditFontSettings => "글꼴 관리…",
            Self::EditControllerSettings => "컨트롤러 설정…",
            Self::EditQuickSettings => "퀵 메뉴 설정…",
            Self::EditToolCycleSettings => "도구 순환 설정…",
            Self::EditWirelessSearch => "무선 장치 검색",
            Self::EditControllerAdd => "새 장치 등록…",
            Self::EditExternalDevice => "외부 장치 연결…",
            Self::EditPrivacySettings => "개인정보 설정…",
            Self::EditSystemServices => "시스템 서비스 설정…",
            Self::EditCanvasBasic => "캔버스 기본 설정…",
            Self::EditCopySvg => "벡터를 SVG로 복사",
            Self::ShowQuickMenu => "퀵 메뉴 열기",
            Self::CycleTools => "다음 도구로 순환",
            Self::EditSuperResolution => "초해상도 확대…",
            Self::EditAutoColor => "전자동 채색",
            Self::EditAutoColorHints => "힌트 이미지로 채색",
            Self::EditAutoColorOptions => "상세 설정으로 채색…",
            Self::EditAutoShade => "자동 음영…",
            Self::EditColorReference => "컬러 참조…",
            Self::EditMaterialImage => "이미지…",
            Self::EditMaterialTemplate => "템플릿…",
            Self::EditTransformScaleRotate => "확대 / 축소 / 회전…",
            Self::EditTransformScale => "확대 / 축소…",
            Self::EditTransformRotate => "회전…",
            Self::EditTransformFree => "자유 변형…",
            Self::EditTransformDistort => "왜곡…",
            Self::EditTransformShear => "평행 왜곡…",
            Self::EditTransformPerspective => "원근 왜곡…",
            Self::EditTransformFlipX => "좌우 반전…",
            Self::EditTransformFlipY => "상하 반전…",
            Self::EditTransformMesh => "메시 변형…",
            Self::EditTransformPins => "퍼펫 변형…",
            Self::EditTransformApply => "변형 확정",
            Self::EditTransformCancel => "변형 취소",
            Self::EditAlignTop => "상단 정렬",
            Self::EditAlignCenterY => "수직 중앙 정렬",
            Self::EditAlignBottom => "하단 정렬",
            Self::EditAlignLeft => "왼쪽 정렬",
            Self::EditAlignCenterX => "수평 중앙 정렬",
            Self::EditAlignRight => "오른쪽 정렬",
            Self::EditDistributeTop => "상단 분포",
            Self::EditDistributeCenterY => "수직 중앙 분포",
            Self::EditDistributeBottom => "하단 분포",
            Self::EditDistributeLeft => "왼쪽 분포",
            Self::EditDistributeCenterX => "수평 중앙 분포",
            Self::EditDistributeRight => "오른쪽 분포",
            Self::EditSpaceY => "수직 등간격 분포",
            Self::EditSpaceX => "수평 등간격 분포",
            Self::EditRefBounds => "정렬 대상 기준",
            Self::EditRefCanvas => "캔버스 기준",
            Self::EditRefSelection => "선택 범위 기준",
            Self::EditRefGuides => "가이드 기준",
            Self::EditRefAuto => "기준 자동 결정",
            Self::EditCut => "잘라내기",
            Self::EditCopy => "복사",
            Self::EditPaste => "붙여넣기",
            Self::EditPasteAtView => "표시 위치에 붙여넣기",
            Self::EditClearOutside => "선택 범위 이외 지우기",
            Self::EditFill => "채우기",
            Self::EditAdvancedFill => "고급 채우기…",
            Self::EditBorder => "선택 범위에 테두리 넣기…",
            Self::EditBrightnessContrast => "밝기 / 대비…",
            Self::EditHsl => "색조 / 채도 / 명도…",
            Self::EditPosterize => "계조화…",
            Self::EditInvert => "계조 반전",
            Self::EditLevels => "레벨 보정…",
            Self::EditToneCurve => "톤 커브…",
            Self::EditColorBalance => "컬러 밸런스…",
            Self::EditThreshold => "2치화…",
            Self::EditGradientMap => "그라데이션 맵…",
            Self::EditResolution => "이미지 해상도 변경…",
            Self::EditCanvasSize => "캔버스 크기 변경…",
            Self::EditCrop => "선택 범위에 맞춰 자르기",
            Self::EditCanvasRotateRight => "시계 방향 90° 회전",
            Self::EditCanvasRotateHalf => "180° 회전",
            Self::EditCanvasRotateLeft => "반시계 방향 90° 회전",
            Self::EditCanvasFlipX => "좌우 반전",
            Self::EditCanvasFlipY => "상하 반전",
            Self::EditClearHistory => "작업 내역 비우기",
            Self::EditPressure => "장치 필압 설정…",
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
            Self::LayerReference => "Set as reference layer",
            Self::LayerDraft => "Set as draft layer",
            Self::LayerDetails => "Layer basic settings…",
            Self::NewLayerFolder => "New layer folder",
            Self::GroupLayersInFolder => "Create folder and insert layers",
            Self::UngroupLayerFolder => "Ungroup layer folder",
            Self::LayerSelectionCreate => "Create selection from layers",
            Self::LayerSelectionAdd => "Add layers to selection",
            Self::LayerSelectionSubtract => "Subtract layers from selection",
            Self::LayerSelectionIntersect => "Intersect selection with layers",
            Self::LayerMaskOutsideSelection => "Mask outside selection",
            Self::LayerMaskSelection => "Mask selection",
            Self::SelectAll => "Select all",
            Self::Deselect => "Deselect",
            Self::InvertSelection => "Invert selection",
        }
    }

    pub fn category(self) -> Category {
        if self.is_native_command() {
            return Category::Layer;
        }
        if self.is_image_edit() {
            return Category::Edit;
        }
        match self {
            Self::SelectAll | Self::Deselect | Self::InvertSelection => {
                Category::Edit
            }
            Self::LayerSelectionCreate
            | Self::LayerSelectionAdd
            | Self::LayerSelectionSubtract
            | Self::LayerSelectionIntersect
            | Self::LayerMaskOutsideSelection
            | Self::LayerMaskSelection => Category::Layer,
            Self::CloseCanvas => Category::File,
            Self::NextCanvas | Self::PreviousCanvas => Category::Window,
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
            Self::LayerReference
            | Self::LayerDraft
            | Self::LayerDetails
            | Self::NewLayerFolder
            | Self::GroupLayersInFolder
            | Self::UngroupLayerFolder => Category::Layer,
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
            Self::BrushSmaller | Self::BrushLarger | Self::SubToolDetail => {
                Category::Options
            }
            Self::ShortcutGuide | Self::Notices => Category::Help,
            _ => Category::View,
        }
    }

    pub(crate) fn is_raster_transform(self) -> bool {
        matches!(
            self,
            Self::EditTransformScaleRotate
                | Self::EditTransformScale
                | Self::EditTransformRotate
                | Self::EditTransformFree
                | Self::EditTransformDistort
                | Self::EditTransformShear
                | Self::EditTransformPerspective
                | Self::EditTransformFlipX
                | Self::EditTransformFlipY
                | Self::EditTransformMesh
                | Self::EditTransformPins
        )
    }
    pub(crate) fn is_settings_command(self) -> bool {
        matches!(
            self,
            Self::EditPreferences
                | Self::EditModifierSettings
                | Self::EditCommandBarSettings
                | Self::EditFontSettings
                | Self::EditControllerSettings
                | Self::EditQuickSettings
                | Self::EditToolCycleSettings
                | Self::EditWirelessSearch
                | Self::EditControllerAdd
                | Self::EditExternalDevice
                | Self::EditPrivacySettings
                | Self::EditSystemServices
                | Self::ShowQuickMenu
                | Self::CycleTools
        )
    }
    pub(crate) fn is_native_command(self) -> bool {
        matches!(
            self,
            Self::NewTextLayer
                | Self::NewVectorLayer
                | Self::EditNativeLayer
                | Self::RasterizeNativeLayer
        )
    }
    pub(crate) fn is_image_edit(self) -> bool {
        if self.is_native_command()
            || matches!(self, Self::EditTextInkBounds | Self::EditVectorCenters)
        {
            return true;
        }
        if matches!(self, Self::EditScreenColor | Self::EditScreenColorHide) {
            return true;
        }
        if self.is_settings_command()
            || matches!(self, Self::EditCanvasBasic | Self::EditCopySvg)
        {
            return true;
        }
        if matches!(
            self,
            Self::EditSuperResolution
                | Self::EditAutoColor
                | Self::EditAutoColorHints
                | Self::EditAutoColorOptions
                | Self::EditAutoShade
                | Self::EditColorReference
                | Self::EditMaterialImage
                | Self::EditMaterialTemplate
        ) {
            return true;
        }
        if self.is_raster_transform()
            || matches!(
                self,
                Self::EditTransformApply | Self::EditTransformCancel
            )
        {
            return true;
        }
        if self.alignment().is_some() || self.align_reference().is_some() {
            return true;
        }
        matches!(
            self,
            Self::EditCut
                | Self::EditCopy
                | Self::EditPaste
                | Self::EditPasteAtView
                | Self::EditClearOutside
                | Self::EditFill
                | Self::EditAdvancedFill
                | Self::EditBorder
                | Self::EditBrightnessContrast
                | Self::EditHsl
                | Self::EditPosterize
                | Self::EditInvert
                | Self::EditLevels
                | Self::EditToneCurve
                | Self::EditColorBalance
                | Self::EditThreshold
                | Self::EditGradientMap
                | Self::EditResolution
                | Self::EditCanvasSize
                | Self::EditCrop
                | Self::EditCanvasRotateRight
                | Self::EditCanvasRotateHalf
                | Self::EditCanvasRotateLeft
                | Self::EditCanvasFlipX
                | Self::EditCanvasFlipY
                | Self::EditClearHistory
                | Self::EditPressure
        )
    }
    pub fn is_tool(self) -> bool {
        self.category() == Category::Tools
    }
    pub(crate) fn raster_conversion(
        self,
        color: [u8; 3],
    ) -> Option<parfait_core::RasterConversion> {
        Some(match self {
            Self::ConvertToDrawingColor => {
                parfait_core::RasterConversion::DrawingColor(color)
            }
            Self::BrightnessToOpacity => {
                parfait_core::RasterConversion::BrightnessToOpacity
            }
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
    pub(crate) fn selection_mode(self) -> Option<parfait_core::SelectionMode> {
        use parfait_core::SelectionMode;
        Some(match self {
            Self::LayerSelectionCreate => SelectionMode::Replace,
            Self::LayerSelectionAdd => SelectionMode::Add,
            Self::LayerSelectionSubtract => SelectionMode::Subtract,
            Self::LayerSelectionIntersect => SelectionMode::Intersect,
            _ => return None,
        })
    }
    pub(crate) fn is_pixel_selection(self) -> bool {
        self.selection_mode().is_some()
            || matches!(
                self,
                Self::SelectAll
                    | Self::Deselect
                    | Self::InvertSelection
                    | Self::LayerMaskOutsideSelection
                    | Self::LayerMaskSelection
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
                Self::BrushSmaller
                    | Self::BrushLarger
                    | Self::HidePalettes
                    | Self::ShortcutGuide
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
        if matches!(command, Command::NewDrawing | Command::OpenProject)
            && (self.documents.is_empty() || self.tabs.enabled)
        {
            if self.dialog_open() {
                return;
            }
            if command == Command::NewDrawing {
                self.new_document();
            } else {
                self.open_project();
            }
            return;
        }
        if self.documents.is_empty() {
            return;
        }
        let target = self.documents.active_session.key;
        self.execute_command_for(target, command, ctx);
    }

    pub(crate) fn execute_command_for(
        &mut self,
        target: crate::document_session::SessionKey,
        command: Command,
        ctx: &egui::Context,
    ) {
        if self.documents.is_empty()
            || self.documents.active_session.key != target
        {
            return;
        }
        if let Some(reason) = self.command_disabled_reason(command) {
            self.notice = Some(reason.into());
            return;
        }
        match command {
            command if command.is_image_edit() => {
                self.execute_image_edit(command, ctx)
            }
            command if command.is_pixel_selection() => {
                self.execute_pixel_selection(command)
            }
            Command::NewDrawing => self.confirm_new = true,
            Command::OpenProject => self.open_project(),
            Command::SaveProject => self.save_project(false),
            Command::SaveAs => self.save_project(true),
            Command::ExportPng => self.export(),
            Command::NextCanvas => self.cycle_canvas(false),
            Command::PreviousCanvas => self.cycle_canvas(true),
            Command::CloseCanvas => {
                if let Some(id) = self.documents.active_id() {
                    self.request_tab_close(id);
                }
            }
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
            | Command::LayerReference
            | Command::LayerDraft
            | Command::LayerClipping => {
                self.execute_selected_layer_command(command)
            }
            Command::LayerDetails => self.open_layer_details(),
            Command::NewLayerFolder
            | Command::GroupLayersInFolder
            | Command::UngroupLayerFolder => {
                self.execute_folder_command(command)
            }
            Command::RenameLayer
            | Command::MoveActiveLayerUp
            | Command::MoveActiveLayerDown
            | Command::MoveActiveLayerToTop
            | Command::MoveActiveLayerToBottom
            | Command::SelectLayerAbove
            | Command::SelectLayerBelow => {
                self.execute_layer_organization(command)
            }
            Command::TransferLayerDown
            | Command::MergeLayerDown
            | Command::MergeSelectedLayers
            | Command::MergeVisibleLayers
            | Command::CopyVisibleLayers
            | Command::FlattenImage
            | Command::ClearLayer => self.combine_layer_command(command),
            Command::ZoomIn => self.documents.active_session.view.step_zoom(1),
            Command::ZoomOut => {
                self.documents.active_session.view.step_zoom(-1)
            }
            Command::ActualSize => self.view_command(ViewCommand::ActualSize),
            Command::DoubleSize => self.view_command(ViewCommand::DoubleSize),
            Command::FitCanvas => self.view_command(ViewCommand::Fit),
            Command::ResetDisplay => self.view_command(ViewCommand::Reset),
            Command::RotateLeft => self.view_command(ViewCommand::RotateLeft),
            Command::RotateRight => self.view_command(ViewCommand::RotateRight),
            Command::ResetRotation => {
                self.view_command(ViewCommand::ResetRotation)
            }
            Command::ResetOrientation => {
                self.view_command(ViewCommand::ResetOrientation)
            }
            Command::FlipHorizontal => {
                self.view_command(ViewCommand::FlipHorizontal)
            }
            Command::FlipVertical => {
                self.view_command(ViewCommand::FlipVertical)
            }
            Command::RulerBar => self.display.rulers = !self.display.rulers,
            Command::PixelGrid => {
                self.display.pixel_grid = !self.display.pixel_grid
            }
            Command::DisplaySettings => self.show_display_settings = true,
            Command::ShortcutSettings => self.open_shortcut_settings(),
            Command::HidePalettes => self.hide_panels = !self.hide_panels,
            Command::ResetPaletteLayout => {
                self.workspace = Default::default();
                ctx.data_mut(|data| {
                    for dock in ["dock_left", "dock_right"] {
                        data.remove::<egui::containers::panel::PanelState>(
                            egui::Id::new(dock),
                        );
                    }
                });
            }
            Command::ToolInk => self.select_brush_group(BrushGroup::Ink),
            Command::ToolSketch => self.select_brush_group(BrushGroup::Sketch),
            Command::ToolPaint => self.select_brush_group(BrushGroup::Paint),
            Command::ToolAirbrush => {
                self.select_brush_group(BrushGroup::Airbrush)
            }
            Command::ToolEraser => self.select_brush_group(BrushGroup::Eraser),
            Command::ToolBlend => self.select_brush_group(BrushGroup::Blend),
            Command::ToolHand => {
                self.choose_tool_category(crate::tools::ToolCategory::Move)
            }
            Command::ToolPicker => {
                self.choose_tool_category(crate::tools::ToolCategory::Picker)
            }
            Command::BrushSmaller => {
                self.brush.size = (self.brush.size / 1.2).max(1.)
            }
            Command::BrushLarger => {
                self.brush.size = (self.brush.size * 1.2).min(512.)
            }
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
            data.insert_temp(
                egui::Id::new(("menu_item", command.label())),
                response.rect,
            )
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

    pub fn command_toggle(
        &mut self,
        ui: &mut egui::Ui,
        command: Command,
        selected: bool,
    ) {
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
                widgets::menu_toggle(
                    ui,
                    label,
                    self.keymap.hint(command),
                    selected,
                )
            })
            .inner;
        #[cfg(test)]
        ui.data_mut(|data| {
            data.insert_temp(
                egui::Id::new(("command_toggle", command)),
                response.rect,
            )
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

    pub fn command_icon(
        &mut self,
        ui: &mut egui::Ui,
        icon: widgets::Icon,
        command: Command,
    ) {
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
