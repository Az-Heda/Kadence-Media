//! Tauri-independent core: SQLite library, folder scanning, tags, playlists,
//! settings and the mpv JSON-IPC client.

mod db;
mod error;
pub mod fileops;
pub mod profiles;
mod rules;
mod stats;
mod transfer;
pub mod models;
pub mod mpv;
pub mod probe;
mod scan;

pub use db::{scan_folder_files_pub, Library, DEFAULT_SETTINGS};
pub use error::{Error, Result};
pub use models::*;
pub use transfer::{ExportFile, ImportReport};
pub use scan::{scan_dir, ScannedFile, VIDEO_EXTENSIONS};
