// app/src-tauri/src/platform/mod.rs — Tout ce qui dépend du système (règle 16.2.6).
//
// macos.rs sert aussi pour Linux (cfg(unix)) ; windows.rs pour Windows.

#[cfg(unix)]
mod macos;
#[cfg(unix)]
pub use macos::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;
