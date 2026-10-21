use std::num::NonZeroU32;

use flowtorrent::{
    domain::{TorrentId, TransferLimits},
    persistence::atomic_write_json,
    rate::parse_rate_mbps,
    settings::{Settings, SettingsStore},
};

#[test]
fn defaults_use_supplied_system_download_folder_without_writing_files() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let settings = Settings::defaults(root.path().into());
    assert_eq!(settings.download_dir, root.path());
    assert!(settings.notify_on_complete);
    assert_eq!(settings.limits, TransferLimits::default());
    let store = SettingsStore::new(root.path().join("settings.json"), root.path().into());
    assert_eq!(store.load()?, settings);
    assert_eq!(std::fs::read_dir(root.path())?.count(), 0);
    Ok(())
}

#[test]
fn decimal_rates_use_megabytes_and_accept_comma_without_float_rounding() -> anyhow::Result<()> {
    for (text, expected) in [
        ("1.5", 1_500_000),
        ("1,5", 1_500_000),
        ("0.02", 20_000),
        ("0,020000", 20_000),
        ("1.000001", 1_000_001),
        ("4294.967295", u32::MAX),
    ] {
        assert_eq!(
            parse_rate_mbps(text, false)?.unwrap().get(),
            expected,
            "{text}"
        );
    }
    assert_eq!(parse_rate_mbps("not a number", true)?, None);
    Ok(())
}

#[test]
fn invalid_or_too_small_rates_are_rejected() {
    for text in [
        "",
        "0",
        "-1",
        "NaN",
        "inf",
        "0.019999",
        "0.000001",
        "4294.967296",
        "1,2.3",
        "1e3",
    ] {
        assert!(parse_rate_mbps(text, false).is_err(), "{text}");
    }
}

#[test]
fn settings_round_trip_and_backup_previous_good_values() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("settings.json");
    let store = SettingsStore::new(path.clone(), root.path().into());
    let first = Settings::defaults(root.path().into());
    store.save(&first)?;
    let next_folder = root.path().join("Загрузки с пробелами");
    std::fs::create_dir(&next_folder)?;
    let second = Settings {
        download_dir: next_folder,
        notify_on_complete: false,
        limits: TransferLimits {
            download_bps: NonZeroU32::new(1_500_000),
            upload_bps: NonZeroU32::new(20_000),
        },
    };
    store.save(&second)?;
    assert_eq!(store.load()?, second);
    let backup_store =
        SettingsStore::new(root.path().join("settings.json.bak"), root.path().into());
    assert_eq!(backup_store.load()?, first);
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    assert_eq!(saved["schema_version"], 1);
    Ok(())
}

#[test]
fn saving_invalid_limit_preserves_previous_settings() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("settings.json");
    let store = SettingsStore::new(path.clone(), root.path().into());
    let mut settings = Settings::defaults(root.path().into());
    store.save(&settings)?;
    let before = std::fs::read(&path)?;
    settings.limits.upload_bps = NonZeroU32::new(19_999);
    assert!(store.save(&settings).is_err());
    assert_eq!(std::fs::read(&path)?, before);
    Ok(())
}

#[test]
fn corrupt_and_future_settings_are_preserved_on_load_and_save() -> anyhow::Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("settings.json");
    let store = SettingsStore::new(path.clone(), root.path().into());
    let settings = Settings::defaults(root.path().into());
    for bytes in [
        b"{ broken json".to_vec(),
        br#"{"schema_version":99,"download_dir":"future"}"#.to_vec(),
    ] {
        std::fs::write(&path, &bytes)?;
        assert!(store.load().is_err());
        assert!(store.save(&settings).is_err());
        assert_eq!(std::fs::read(&path)?, bytes);
    }
    Ok(())
}

#[test]
fn failed_serialization_does_not_replace_the_last_good_file() -> anyhow::Result<()> {
    struct Broken;
    impl serde::Serialize for Broken {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("injected serialization failure"))
        }
    }
    let root = tempfile::tempdir()?;
    let path = root.path().join("state.json");
    atomic_write_json(&path, &serde_json::json!({"value": 42}))?;
    let before = std::fs::read(&path)?;
    assert!(atomic_write_json(&path, &Broken).is_err());
    assert_eq!(std::fs::read(&path)?, before);
    assert_eq!(std::fs::read_dir(root.path())?.count(), 1);
    Ok(())
}

#[test]
fn torrent_id_uses_validated_forty_character_hex_in_saved_records() -> anyhow::Result<()> {
    let id = TorrentId([0xab; 20]);
    assert_eq!(
        serde_json::to_string(&id)?,
        "\"abababababababababababababababababababab\""
    );
    assert_eq!(
        serde_json::from_str::<TorrentId>("\"ABABABABABABABABABABABABABABABABABABABAB\"")?,
        id
    );
    for invalid in [
        "\"\"",
        "\"1234\"",
        "\"zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz\"",
    ] {
        assert!(serde_json::from_str::<TorrentId>(invalid).is_err());
    }
    Ok(())
}

// Child-process entry point for the interrupted-write test below. With no
// explicit test fixture directory this does no work in the normal test run.
#[test]
fn atomic_write_worker() -> anyhow::Result<()> {
    let Some(root) = std::env::var_os("FLOWTORRENT_TEST_ATOMIC_DIR") else {
        return Ok(());
    };
    let path = std::path::PathBuf::from(root).join("atomic.json");
    let payload = "x".repeat(2_097_152);
    for generation in 0..100 {
        atomic_write_json(
            &path,
            &serde_json::json!({"generation":generation,"payload":payload}),
        )?;
    }
    Ok(())
}

#[test]
fn interrupted_write_keeps_complete_previous_or_next_json() -> anyhow::Result<()> {
    use std::{
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let root = tempfile::tempdir()?;
    let path = root.path().join("atomic.json");
    let mut child = OwnedChild(
        Command::new(std::env::current_exe()?)
            .args(["--exact", "atomic_write_worker", "--nocapture"])
            .env("FLOWTORRENT_TEST_ATOMIC_DIR", root.path())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?,
    );
    let start = Instant::now();
    loop {
        let writing_temp = std::fs::read_dir(root.path())?
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with(".tmp"));
        if path.exists() && writing_temp {
            break;
        }
        anyhow::ensure!(
            child.0.try_wait()?.is_none(),
            "writer exited before an in-progress replacement was observed"
        );
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(30),
            "timed out waiting for an in-progress replacement"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    child.0.kill()?;
    child.0.wait()?;
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
    assert!(value["generation"].as_u64().is_some());
    let payload = value["payload"].as_str().unwrap();
    assert_eq!(payload.len(), 2_097_152);
    assert!(payload.bytes().all(|b| b == b'x'));
    let backup = root.path().join("atomic.json.bak");
    if backup.exists() {
        let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(backup)?)?;
        assert_eq!(saved["payload"].as_str().unwrap().len(), 2_097_152);
    }
    Ok(())
}
