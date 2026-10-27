use crate::{
    error::AppResult,
    persistence::{atomic_write_json, check_schema},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Serialize, Deserialize)]
struct ShellState {
    schema_version: u32,
    background_explained: bool,
}
pub struct BackgroundNotice {
    path: PathBuf,
    state: ShellState,
}
impl BackgroundNotice {
    pub fn load(path: PathBuf) -> AppResult<Self> {
        let state = match std::fs::read(&path) {
            Ok(bytes) => {
                check_schema(&path, &bytes)?;
                serde_json::from_slice(&bytes)?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => ShellState {
                schema_version: 1,
                background_explained: false,
            },
            Err(error) => return Err(error.into()),
        };
        Ok(Self { path, state })
    }
    pub fn was_shown(&self) -> bool {
        self.state.background_explained
    }
    pub fn mark_shown(&mut self) -> AppResult<()> {
        let next = ShellState {
            schema_version: 1,
            background_explained: true,
        };
        atomic_write_json(&self.path, &next)?;
        self.state = next;
        Ok(())
    }
}
