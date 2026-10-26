pub mod commands;
mod controller;
pub mod lifecycle;
pub mod preview;
pub mod restore;
pub mod storage;
pub mod targets;
pub use controller::{AppController, AppEvent, AppHandle, AppSnapshot};

pub mod trash;
