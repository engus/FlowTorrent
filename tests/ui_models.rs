use flowtorrent::{
    domain::*,
    ui::{
        i18n::{Locale, state_label},
        models::{PreviewSelection, to_row},
    },
};
use std::path::PathBuf;

#[path = "support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod support;

#[tokio::test]
async fn settings_completion_is_sent_after_durable_save_and_snapshot_update() -> anyhow::Result<()>
{
    use flowtorrent::{
        app::{
            AppEvent,
            commands::{AppCommand, AppOperation},
            storage::SessionStore,
        },
        settings::{Settings, SettingsStore},
    };
    use std::{sync::Arc, time::Duration};
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let mut handle =
        lifecycle::start(root.path(), Arc::new(SessionStore::new(paths.clone()))).await?;
    let mut settings = Settings::defaults(paths.download_dir.clone());
    settings.notify_on_complete = false;
    handle
        .commands
        .send(AppCommand::UpdateSettings(settings.clone()))
        .await?;
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let Some(AppEvent::CommandFinished {
                operation: AppOperation::UpdateSettings,
                success: true,
            }) = handle.events.recv().await
            {
                break;
            }
        }
    })
    .await?;
    assert_eq!(handle.snapshots.borrow().settings, settings);
    let stored = SettingsStore::new(paths.config_file.clone(), paths.download_dir.clone());
    assert_eq!(stored.load()?, settings);
    let mut invalid = settings.clone();
    invalid.download_dir = root.path().join("missing");
    handle
        .commands
        .send(AppCommand::UpdateSettings(invalid))
        .await?;
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let Some(AppEvent::CommandFinished {
                operation: AppOperation::UpdateSettings,
                success: false,
            }) = handle.events.recv().await
            {
                break;
            }
        }
    })
    .await?;
    assert_eq!(stored.load()?, settings);
    lifecycle::stop(handle).await
}

#[test]
fn locale_and_state_strings_are_exact_in_russian_and_english() {
    for tag in ["ru", "ru-RU", "RU_ru"] {
        assert_eq!(Locale::from_system_tag(tag), Locale::Ru);
    }
    for tag in ["en-US", "fr", "rue", ""] {
        assert_eq!(Locale::from_system_tag(tag), Locale::En);
    }
    let states = [
        TransferState::Checking,
        TransferState::Connecting,
        TransferState::Downloading,
        TransferState::WaitingForSources,
        TransferState::Paused,
        TransferState::Seeding,
        TransferState::Completed,
        TransferState::Error,
    ];
    let ru = [
        "Проверка",
        "Подключение",
        "Скачивается",
        "Ожидание источников",
        "Пауза",
        "Раздаётся",
        "Завершено",
        "Ошибка",
    ];
    let en = [
        "Checking",
        "Connecting",
        "Downloading",
        "Waiting for sources",
        "Paused",
        "Seeding",
        "Completed",
        "Error",
    ];
    for ((state, ru), en) in states.into_iter().zip(ru).zip(en) {
        assert_eq!(state_label(state, Locale::Ru), ru);
        assert_eq!(state_label(state, Locale::En), en);
    }
}

#[test]
fn selected_progress_labels_and_paths_do_not_change_filename_or_intent() {
    let mut row = TorrentSnapshot {
        id: TorrentId([1; 20]),
        name: "Мой.File.tar".into(),
        selected_bytes: 1_500_000,
        checked_downloaded_bytes: 750_000,
        download_bps: 1_500_000,
        upload_bps: 20_000,
        session_uploaded_bytes: 0,
        state: TransferState::Downloading,
        error: None,
        destination: PathBuf::from("chosen/folder"),
        desired_state: DesiredState::Active,
    };
    let model = to_row(&row, Locale::Ru);
    assert_eq!(model.name, "Мой.File.tar");
    assert_eq!(model.progress_permille, 500);
    assert_eq!(model.volume, "750 КБ / 1,5 МБ");
    assert_eq!(model.download, "1,5 МБ/с");
    assert_eq!(model.destination, PathBuf::from("chosen/folder"));
    assert_eq!(model.action, "Пауза");
    row.state = TransferState::Completed;
    row.desired_state = DesiredState::Paused;
    row.checked_downloaded_bytes = 1_500_000;
    let model = to_row(&row, Locale::En);
    assert_eq!(model.status, "Completed");
    assert_eq!(model.action, "Resume");
    assert_eq!(model.progress_permille, 1000);
    row.state = TransferState::Checking;
    assert_eq!(to_row(&row, Locale::Ru).action, "Продолжить");
    row.selected_bytes = 0;
    row.checked_downloaded_bytes = u64::MAX;
    assert_eq!(to_row(&row, Locale::Ru).progress_permille, 0);
}

#[test]
fn padding_is_hidden_without_renumbering_and_no_files_disables_confirmation() {
    let preview = TorrentPreview {
        id: TorrentId([2; 20]),
        name: "bundle".into(),
        multi_file: true,
        metadata: vec![],
        files: vec![
            TorrentFile {
                id: FileId(0),
                relative_path: "a.bin".into(),
                length: 10,
                is_padding: false,
            },
            TorrentFile {
                id: FileId(1),
                relative_path: ".pad/6".into(),
                length: 6,
                is_padding: true,
            },
            TorrentFile {
                id: FileId(2),
                relative_path: "b.bin".into(),
                length: 20,
                is_padding: false,
            },
        ],
    };
    let mut selection = PreviewSelection::new(&std::sync::Arc::new(preview));
    assert_eq!(
        selection.files().map(|file| file.id).collect::<Vec<_>>(),
        vec![FileId(0), FileId(2)]
    );
    assert_eq!(selection.selected_bytes(), 30);
    assert!(selection.can_confirm());
    selection.set_checked(FileId(0), false);
    selection.set_checked(FileId(2), false);
    assert!(!selection.can_confirm());
    assert_eq!(selection.selected_bytes(), 0);
    selection.set_checked(FileId(1), true);
    assert!(!selection.can_confirm());
    selection.set_checked(FileId(2), true);
    assert_eq!(selection.selected_files(), vec![FileId(2)]);
}
