#[path = "support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod support;
use flowtorrent::app::storage::{SessionRepository, SessionStore};
#[test]
fn priority_manifest_v3_roundtrips_and_rejects_bad_overrides_without_overwrite()
-> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.data_dir)?;
    let id = "0404040404040404040404040404040404040404";
    let value = serde_json::json!({"schema_version":3,"next_position":1,"torrents":[{
        "id":id,"name":"bundle","position":0,"metadata_path":paths.metadata_dir.join(format!("{id}.torrent")),
        "base_dir":paths.download_dir,"selected_files":[0,1],"file_priorities":[{"file":1,"priority":"high"}],
        "desired_state":"paused","completion_notified":false}]});
    let bytes = serde_json::to_vec(&value)?;
    std::fs::write(&paths.manifest_file, &bytes)?;
    let store = SessionStore::new(paths.clone());
    let manifest = store.load()?;
    store.save(&manifest)?;
    assert_eq!(
        serde_json::to_value(&manifest)?["torrents"][0]["file_priorities"],
        value["torrents"][0]["file_priorities"]
    );
    for overrides in [
        serde_json::json!([{"file":1,"priority":"urgent"}]),
        serde_json::json!([{"file":1,"priority":"high"},{"file":1,"priority":"low"}]),
        serde_json::json!([{"file":1,"priority":"normal"}]),
    ] {
        let mut bad = value.clone();
        bad["torrents"][0]["file_priorities"] = overrides;
        let bytes = serde_json::to_vec(&bad)?;
        std::fs::write(&paths.manifest_file, &bytes)?;
        assert!(store.load().is_err());
        assert!(store.save(&manifest).is_err());
        assert_eq!(std::fs::read(&paths.manifest_file)?, bytes);
    }
    Ok(())
}
#[test]
fn legacy_manifests_mean_normal_and_future_schema_stays_untouched() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.data_dir)?;
    let id = "0404040404040404040404040404040404040404";
    for version in [1, 2] {
        let value = serde_json::json!({"schema_version":version,"next_position":1,"torrents":[{"id":id,"name":"bundle","position":0,"metadata_path":paths.metadata_dir.join(format!("{id}.torrent")),"base_dir":paths.download_dir,"selected_files":[0],"desired_state":"paused","completion_notified":false}]});
        std::fs::write(&paths.manifest_file, serde_json::to_vec(&value)?)?;
        let store = SessionStore::new(paths.clone());
        let manifest = store.load()?;
        assert!(manifest.torrents[0].file_priorities.is_empty());
        store.save(&manifest)?;
        assert_eq!(store.load()?.schema_version, version);
    }
    let bytes = br#"{"schema_version":5}"#;
    std::fs::write(&paths.manifest_file, bytes)?;
    let store = SessionStore::new(paths.clone());
    assert!(store.save(&Default::default()).is_err());
    assert_eq!(std::fs::read(&paths.manifest_file)?, bytes);
    Ok(())
}

#[test]
fn torrent_priority_v4_roundtrips_legacy_defaults_and_rejects_downgrade() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    std::fs::create_dir_all(&paths.data_dir)?;
    let id = "0404040404040404040404040404040404040404";
    let mut value = serde_json::json!({"schema_version":4,"next_position":1,"torrents":[{
        "id":id,"name":"bundle","position":0,"metadata_path":paths.metadata_dir.join(format!("{id}.torrent")),
        "base_dir":paths.download_dir,"selected_files":[0],"priority":"high",
        "desired_state":"paused","completion_notified":false}]});
    std::fs::write(&paths.manifest_file, serde_json::to_vec(&value)?)?;
    let store = SessionStore::new(paths.clone());
    let manifest = store.load()?;
    assert_eq!(
        serde_json::to_value(&manifest)?["torrents"][0]["priority"],
        "high"
    );
    store.save(&manifest)?;
    for bad in [
        serde_json::json!("urgent"),
        serde_json::json!(3),
        serde_json::Value::Null,
    ] {
        value["torrents"][0]["priority"] = bad;
        let bytes = serde_json::to_vec(&value)?;
        std::fs::write(&paths.manifest_file, &bytes)?;
        assert!(store.load().is_err());
        assert!(store.save(&manifest).is_err());
        assert_eq!(std::fs::read(&paths.manifest_file)?, bytes);
    }
    for schema in 1..=3 {
        value["schema_version"] = schema.into();
        value["torrents"][0]
            .as_object_mut()
            .unwrap()
            .remove("priority");
        std::fs::write(&paths.manifest_file, serde_json::to_vec(&value)?)?;
        let legacy = store.load()?;
        assert_eq!(
            serde_json::to_value(&legacy)?["torrents"][0]["priority"],
            "normal"
        );
        value["torrents"][0]["priority"] = "high".into();
        std::fs::write(&paths.manifest_file, serde_json::to_vec(&value)?)?;
        assert!(store.load().is_err(), "older schema accepted new semantics");
    }
    Ok(())
}
