pub mod traits;

pub use traits::create_capture;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub mod unsupported;

#[cfg(target_os = "macos")]
pub use macos::MacosBackend as Backend;

#[cfg(target_os = "windows")]
pub use windows::WindowsBackend as Backend;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub use unsupported::UnsupportedBackend as Backend;
