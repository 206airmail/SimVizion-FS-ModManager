//! Core library for SimVizion Mod Manager.
//!
//! Everything that touches Farming Simulator files lives here, free of UI and
//! Tauri dependencies, so it can be tested against fixtures in temp folders.
//! Every function takes the paths it works on as parameters.

pub mod container;
pub mod discovery;
pub mod files;
pub mod icons;
pub mod library;
pub mod model;
pub mod net;
pub mod ops;
pub mod savegame;
pub mod scan;
pub mod store;

#[cfg(test)]
pub(crate) mod testutil;
