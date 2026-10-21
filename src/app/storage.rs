use crate::{domain::Manifest, error::AppResult};

pub trait SessionRepository: Send + Sync {
    fn load(&self) -> AppResult<Manifest>;
    fn save(&self, manifest: &Manifest) -> AppResult<()>;
}
