use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi(object)]
#[derive(Debug, Clone)]
pub struct ScreenshotConfig {
    pub save_path: Option<String>,
    pub format: Option<String>,
    pub quality: Option<u32>,
    pub mosaic_block_size: Option<u32>,
    pub default_color: Option<String>,
    pub default_size: Option<u32>,
    pub locale: Option<String>,
    pub show_debug_hud: Option<bool>,
    pub metrics_interval_ms: Option<u32>,
}

impl Default for ScreenshotConfig {
    fn default() -> Self {
        Self {
            save_path: None,
            format: Some("png".into()),
            quality: Some(90),
            mosaic_block_size: Some(8),
            default_color: Some("#ff0000".into()),
            default_size: Some(3),
            locale: Some("zh-CN".into()),
            show_debug_hud: Some(false),
            metrics_interval_ms: Some(500),
        }
    }
}

impl ScreenshotConfig {
    pub fn merge(self) -> Self {
        let default = Self::default();
        Self {
            save_path: self.save_path.or(default.save_path),
            format: self.format.or(default.format),
            quality: self.quality.or(default.quality),
            mosaic_block_size: self.mosaic_block_size.or(default.mosaic_block_size),
            default_color: self.default_color.or(default.default_color),
            default_size: self.default_size.or(default.default_size),
            locale: self.locale.or(default.locale),
            show_debug_hud: self.show_debug_hud.or(default.show_debug_hud),
            metrics_interval_ms: self.metrics_interval_ms.or(default.metrics_interval_ms),
        }
    }
}
