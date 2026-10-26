use crate::domain::{TorrentId, TorrentSnapshot, TransferState};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Category {
    #[default]
    All,
    Incomplete,
    Downloaded,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Added,
    Name,
    Size,
    Progress,
    Speed,
}
#[derive(Default)]
pub struct ViewState {
    pub category: Category,
    pub search: String,
    pub search_open: bool,
    pub selecting: bool,
    pub selected: BTreeSet<TorrentId>,
    pub sort: Sort,
    anchor: Option<TorrentId>,
}
pub fn complete(row: &TorrentSnapshot) -> bool {
    matches!(row.state, TransferState::Completed | TransferState::Seeding)
        || row.selected_bytes > 0 && row.checked_downloaded_bytes >= row.selected_bytes
}
impl ViewState {
    pub fn checkboxes(&self) -> bool {
        self.search_open || self.selecting
    }
    pub fn single(&self) -> Option<TorrentId> {
        (self.selected.len() == 1).then(|| *self.selected.first().unwrap())
    }
    pub fn click(&mut self, id: TorrentId, order: &[TorrentId], toggle: bool, range: bool) {
        if !order.contains(&id) {
            return;
        }
        if range {
            self.selecting = true;
            if let Some(a) = self
                .anchor
                .and_then(|id| order.iter().position(|v| *v == id))
            {
                let b = order.iter().position(|v| *v == id).unwrap();
                self.selected = order[a.min(b)..=a.max(b)].iter().copied().collect();
            } else {
                self.selected.insert(id);
            }
        } else if toggle {
            self.selecting = true;
            if !self.selected.remove(&id) {
                self.selected.insert(id);
            }
        } else {
            self.selected.clear();
            self.selected.insert(id);
        }
        if !range {
            self.anchor = Some(id);
        }
    }
    pub fn reconcile(&mut self, visible: &[TorrentId]) {
        self.selected.retain(|id| visible.contains(id));
        if self.anchor.is_some_and(|id| !visible.contains(&id)) {
            self.anchor = None;
        }
    }
    pub fn visible(&self, rows: &[TorrentSnapshot]) -> Vec<TorrentId> {
        let query = self.search.trim().to_lowercase();
        let mut rows: Vec<_> = rows
            .iter()
            .filter(|r| {
                (match self.category {
                    Category::All => true,
                    Category::Incomplete => !complete(r),
                    Category::Downloaded => complete(r),
                }) && (query.is_empty() || r.name.to_lowercase().contains(&query))
            })
            .collect();
        match self.sort {
            Sort::Added => {}
            Sort::Name => rows.sort_by_cached_key(|r| r.name.to_lowercase()),
            Sort::Size => rows.sort_by_key(|r| std::cmp::Reverse(r.selected_bytes)),
            Sort::Progress => rows.sort_by_key(|r| {
                std::cmp::Reverse(if r.selected_bytes == 0 {
                    0
                } else {
                    u128::from(r.checked_downloaded_bytes.min(r.selected_bytes)) * 1000
                        / u128::from(r.selected_bytes)
                })
            }),
            Sort::Speed => rows.sort_by_key(|r| {
                std::cmp::Reverse(if complete(r) {
                    r.upload_bps
                } else {
                    r.download_bps
                })
            }),
        }
        rows.into_iter().map(|r| r.id).collect()
    }
}
pub fn eta_seconds(total: u64, checked: u64, speed: u64) -> Option<u64> {
    let remaining = total.saturating_sub(checked);
    if remaining == 0 {
        Some(0)
    } else if speed == 0 {
        None
    } else {
        Some(remaining.div_ceil(speed))
    }
}
pub fn uploaded_percent(uploaded: u64, downloaded: u64) -> Option<u64> {
    (downloaded > 0).then(|| {
        (u128::from(uploaded) * 100 / u128::from(downloaded)).min(u128::from(u64::MAX)) as u64
    })
}
