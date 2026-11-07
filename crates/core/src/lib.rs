//! Core of DebridDock: provider adapters, a resumable downloader and the queue engine.
//! It has no UI or Tauri dependency, so it builds and its tests run on every platform.

pub mod downloader;
pub mod engine;
pub mod error;
pub mod models;
pub mod parser;
pub mod paths;
pub mod provider;
pub mod search;
pub mod store;
pub mod stream;

pub use error::{Error, Result};
