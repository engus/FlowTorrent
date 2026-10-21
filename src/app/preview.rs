use crate::domain::TorrentId;
use std::{
    collections::{BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub struct PreviewQueue {
    paths: VecDeque<PathBuf>,
}

impl PreviewQueue {
    pub fn enqueue(&mut self, path: PathBuf) {
        self.paths.push_back(path);
    }
    pub fn current_path(&self) -> Option<&Path> {
        self.paths.front().map(PathBuf::as_path)
    }
    pub fn cancel_current(&mut self) {
        self.paths.pop_front();
    }
}

#[derive(Clone, Default)]
pub struct AddReservations {
    ids: Arc<Mutex<BTreeSet<TorrentId>>>,
}

pub struct AddReservation {
    owner: AddReservations,
    id: TorrentId,
    committed: bool,
}

impl AddReservations {
    pub fn reserve(&self, id: TorrentId) -> Option<AddReservation> {
        if !self.ids.lock().unwrap().insert(id) {
            return None;
        }
        Some(AddReservation {
            owner: self.clone(),
            id,
            committed: false,
        })
    }
    pub fn remove(&self, id: TorrentId) {
        self.ids.lock().unwrap().remove(&id);
    }
}

impl AddReservation {
    pub fn commit(mut self) {
        self.committed = true;
    }
}
impl Drop for AddReservation {
    fn drop(&mut self) {
        if !self.committed {
            self.owner.remove(self.id);
        }
    }
}
