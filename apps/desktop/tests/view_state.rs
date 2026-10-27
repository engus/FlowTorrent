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
        checking: None,
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
fn context_target_preserves_selected_group_and_retargets_by_stable_id() {
    let ids = [TorrentId([1; 20]), TorrentId([2; 20]), TorrentId([3; 20])];
    let mut view = ViewState::default();
    view.click(ids[0], &ids, false, false);
    view.click(ids[1], &ids, true, false);
    let selected = view.selected.clone();
    assert!(view.context_target(ids[0], &[ids[2], ids[1], ids[0]]));
    assert_eq!(view.selected, selected);
    assert!(!view.details_open);
    assert!(view.context_target(ids[2], &ids));
    assert_eq!(view.single(), Some(ids[2]));
    assert!(!view.context_target(TorrentId([99; 20]), &ids));
    assert_eq!(view.single(), Some(ids[2]));
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

#[test]
fn drawer_inspection_is_explicit_and_closing_keeps_selection() {
    let ids = [TorrentId([1; 20]), TorrentId([2; 20])];
    let mut view = ViewState::default();
    view.click(ids[0], &ids, false, false);
    assert_eq!(view.inspected(), None);
    view.toggle_details(ids[0], &ids);
    assert_eq!(view.inspected(), Some(ids[0]));
    view.click(ids[1], &ids, false, false);
    assert_eq!(view.inspected(), Some(ids[1]));
    view.toggle_details(ids[1], &ids);
    assert_eq!(view.inspected(), None);
    assert_eq!(view.single(), Some(ids[1]));
    assert!(!view.checkboxes());
    view.toggle_details(ids[0], &ids);
    view.click(ids[1], &ids, true, false);
    assert_eq!(
        view.inspected(),
        None,
        "multi-selection requested unbounded file details"
    );
    view.reconcile(&[]);
    assert!(!view.details_open);
    view.click(ids[0], &ids, false, false);
    assert_eq!(
        view.inspected(),
        None,
        "filter/removal reopened a closed drawer"
    );
}

#[test]
fn downloading_and_completed_labels_keep_unfinished_and_seeding_membership() {
    use flowtorrent::ui::i18n::{Locale, category_label};
    assert_eq!(
        category_label(Category::Incomplete, Locale::En),
        "Downloading"
    );
    assert_eq!(
        category_label(Category::Downloaded, Locale::En),
        "Completed"
    );
    let mut paused = row(11, false);
    paused.state = TransferState::Paused;
    let mut waiting = row(12, false);
    waiting.state = TransferState::WaitingForSources;
    let mut active = row(13, false);
    active.state = TransferState::Downloading;
    let mut seeding = row(14, true);
    seeding.state = TransferState::Seeding;
    let mut error = row(15, false);
    error.state = TransferState::Error;
    let rows = [paused, waiting, active, seeding, error];
    let mut view = ViewState::default();
    view.category = Category::Incomplete;
    assert_eq!(
        view.visible(&rows),
        vec![rows[0].id, rows[1].id, rows[2].id, rows[4].id]
    );
    view.category = Category::Downloaded;
    assert_eq!(view.visible(&rows), vec![rows[3].id]);
}
