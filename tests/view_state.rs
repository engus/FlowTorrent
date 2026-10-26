use flowtorrent::{
    domain::*,
    ui::view_state::{Category, ViewState, eta_seconds, uploaded_percent},
};
use std::path::PathBuf;
fn row(n: u8, complete: bool) -> TorrentSnapshot {
    TorrentSnapshot {
        id: TorrentId([n; 20]),
        name: format!("Item {n}"),
        destination: PathBuf::new(),
        selected_bytes: 100,
        checked_downloaded_bytes: if complete { 100 } else { 25 },
        download_bps: 10,
        upload_bps: 0,
        session_uploaded_bytes: 0,
        state: TransferState::Paused,
        desired_state: DesiredState::Paused,
        error: None,
    }
}
#[test]
fn ordinary_click_inspects_without_checkboxes_and_modifier_selects_ids() {
    let rows = [row(1, false), row(2, false), row(3, true)];
    let ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    let mut view = ViewState::default();
    view.click(ids[0], &ids, false, false);
    assert!(!view.checkboxes());
    assert_eq!(view.single(), Some(ids[0]));
    view.click(ids[2], &ids, false, true);
    assert_eq!(view.selected.len(), 3);
    assert!(view.checkboxes());
    view.click(ids[1], &ids, true, false);
    assert_eq!(view.selected.len(), 2);
    view.click(ids[0], &ids, false, false);
    assert_eq!(view.single(), Some(ids[0]));
}
#[test]
fn filter_search_and_removal_reconcile_detail_selection() {
    let rows = vec![row(1, false), row(2, true)];
    let mut view = ViewState::default();
    view.selected.insert(rows[0].id);
    view.category = Category::Downloaded;
    let visible = view.visible(&rows);
    assert_eq!(visible, vec![rows[1].id]);
    view.reconcile(&visible);
    assert_eq!(view.single(), None);
    view.search = "ITEM 2".into();
    assert_eq!(view.visible(&rows), visible);
    view.search = "absent".into();
    assert!(view.visible(&rows).is_empty());
}
#[test]
fn eta_and_ratio_are_safe_and_do_not_cap_seeding_at_one_hundred_percent() {
    assert_eq!(eta_seconds(100, 25, 10), Some(8));
    assert_eq!(eta_seconds(100, 25, 0), None);
    assert_eq!(eta_seconds(100, 200, 0), Some(0));
    assert_eq!(uploaded_percent(250, 100), Some(250));
    assert_eq!(uploaded_percent(12, 0), None);
    assert_eq!(uploaded_percent(u64::MAX, 1), Some(u64::MAX));
}
