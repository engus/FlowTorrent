use flowtorrent::{settings::LanguagePreference, ui::i18n::Locale};
#[test]
fn locale_detection_and_saved_choices_cover_all_fourteen_languages() {
    assert_eq!(Locale::ALL.len(), 14);
    for (tag, locale) in [
        ("es-MX", Locale::Es),
        ("PT_br", Locale::Pt),
        ("fr-CA", Locale::Fr),
        ("de-DE", Locale::De),
        ("it-IT", Locale::It),
        ("pl-PL", Locale::Pl),
        ("tr-TR", Locale::Tr),
        ("ja-JP", Locale::Ja),
        ("ko-KR", Locale::Ko),
        ("zh_Hans_CN", Locale::ZhCn),
        ("kk-KZ", Locale::Kk),
        ("uk-UA", Locale::Uk),
    ] {
        assert_eq!(Locale::from_system_tag(tag), locale);
        assert_eq!(Locale::from_index(locale.index()), Some(locale));
    }
    for tag in ["zh-TW", "zh-Hant", "ar-SA", "unknown", ""] {
        assert_eq!(Locale::from_system_tag(tag), Locale::En);
    }
    assert_eq!(
        serde_json::from_str::<LanguagePreference>("\"ru\"").unwrap(),
        LanguagePreference::Ru
    );
    assert_eq!(
        serde_json::from_str::<LanguagePreference>("\"en\"").unwrap(),
        LanguagePreference::En
    );
    assert_eq!(
        serde_json::from_str::<LanguagePreference>("\"system\"").unwrap(),
        LanguagePreference::System
    );
    for preference in LanguagePreference::ALL {
        let json = serde_json::to_string(&preference).unwrap();
        assert_eq!(
            serde_json::from_str::<LanguagePreference>(&json).unwrap(),
            preference
        );
    }
}
#[test]
fn contextual_translations_and_unknown_message_fallback_are_safe() {
    assert_eq!(Locale::Es.text("Скачать", "Download"), "Descargar");
    assert_eq!(Locale::Es.text("Скачивание", "Download"), "Descarga");
    assert_eq!(Locale::Uk.text("Завершённые", "Completed"), "Завершені");
    assert_eq!(Locale::Kk.text("Закрыть", "Close"), "Жабу");
    assert_eq!(Locale::Ja.text("Настройки", "Settings"), "設定");
    assert_eq!(
        Locale::De.text("Новая неизвестная строка", "Uncatalogued message"),
        "Uncatalogued message"
    );
    assert_eq!(
        Locale::Ru.text("Новая неизвестная строка", "Uncatalogued message"),
        "Новая неизвестная строка"
    );
}

#[test]
fn every_language_survives_the_actual_settings_store_without_changing_transfers() {
    use flowtorrent::settings::{Settings, SettingsStore};
    let folder = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(
        folder.path().join("settings.json"),
        folder.path().to_owned(),
    );
    let mut settings = Settings::defaults(folder.path().to_owned());
    for language in LanguagePreference::ALL {
        settings.ui.language = language;
        store.save(&settings).unwrap();
        assert_eq!(store.load().unwrap(), settings);
        if let Some(tag) = language.tag() {
            assert_eq!(
                Locale::from_preference(language, Locale::Ru),
                Locale::from_system_tag(tag)
            );
        }
    }
}
