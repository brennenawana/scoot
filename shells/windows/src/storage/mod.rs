//! Everything Scoot persists, all of it under `%APPDATA%\Scoot\` with the
//! same file names macOS and Linux use (CONTRACTS.md §8).

pub mod events;
pub mod paths;
pub mod settings;

pub use events::EventLog;
pub use paths::app_data_dir;
pub use settings::OverlayCorner;
