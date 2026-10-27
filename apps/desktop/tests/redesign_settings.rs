use flowtorrent::{
    rate::{parse_rate_mbit, rate_mbit_text},
    settings::{LanguagePreference, Settings, ThemePreference},
};
#[test]
fn mbit_limits_use_exact_decimal_units_and_new_minimum() {
    for (text, value) in [
        ("0.5", 62500),
        ("0,5", 62500),
        ("1", 125000),
        ("8", 1000000),
        ("1.000008", 125001),
    ] {
        assert_eq!(parse_rate_mbit(text, false).unwrap().unwrap().get(), value);
        assert_eq!(
            parse_rate_mbit(&rate_mbit_text(std::num::NonZeroU32::new(value)), false)
                .unwrap()
                .unwrap()
                .get(),
            value
        );
    }
    for text in ["0.49", "0", "NaN", "-1", "1e3", "1.000001", "9999999999"] {
        assert!(parse_rate_mbit(text, false).is_err(), "{text}");
    }
    assert_eq!(parse_rate_mbit("invalid", true).unwrap(), None);
}
#[test]
fn original_settings_migrate_without_changing_limits_or_enabling_login() {
    let root = tempfile::tempdir().unwrap();
    let original = serde_json::json!({"download_dir":root.path(),"notify_on_complete":false,"limits":{"download_bps":20000,"upload_bps":null}});
    let settings: Settings = serde_json::from_value(original).unwrap();
    assert_eq!(settings.ui.theme, ThemePreference::System);
    assert_eq!(settings.ui.language, LanguagePreference::System);
    assert!(!settings.ui.launch_at_login);
    assert_eq!(settings.limits.download_bps.unwrap().get(), 20000);
    assert!(!settings.notify_on_complete);
}
#[test]
fn minimum_limit_equivalent_does_not_round_away_half_a_kilobyte() {
    assert_eq!(
        flowtorrent::ui::i18n::format_rate_equivalent(62_500, flowtorrent::ui::i18n::Locale::En),
        "62.5 KB/s"
    );
    assert_eq!(
        flowtorrent::ui::i18n::format_rate_equivalent(62_500, flowtorrent::ui::i18n::Locale::Ru),
        "62,5 КБ/с"
    );
}

#[test]
fn tray_preference_defaults_on_for_old_settings_and_disabled_roundtrips() {
    use flowtorrent::settings::SettingsStore;
    let root = tempfile::tempdir().unwrap();
    for ui in [
        serde_json::json!({}),
        serde_json::json!({"language":"en","theme":"dark"}),
    ] {
        let old = serde_json::json!({"download_dir":root.path(),"notify_on_complete":false,"limits":{"download_bps":null,"upload_bps":null},"ui":ui});
        let settings: Settings = serde_json::from_value(old).unwrap();
        assert!(settings.ui.show_tray_icon);
    }
    let store = SettingsStore::new(root.path().join("settings.json"), root.path().to_owned());
    let mut settings = store.load().unwrap();
    assert!(settings.ui.show_tray_icon);
    settings.ui.show_tray_icon = false;
    store.save(&settings).unwrap();
    assert!(!store.load().unwrap().ui.show_tray_icon);
}
