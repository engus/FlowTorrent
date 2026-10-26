use crate::domain::{ErrorCode, TransferState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Locale {
    Ru,
    En,
    Es,
    Pt,
    Fr,
    De,
    It,
    Pl,
    Tr,
    Ja,
    Ko,
    ZhCn,
    Kk,
    Uk,
}
impl Locale {
    pub const ALL: [Self; 14] = [
        Self::Ru,
        Self::En,
        Self::Es,
        Self::Pt,
        Self::Fr,
        Self::De,
        Self::It,
        Self::Pl,
        Self::Tr,
        Self::Ja,
        Self::Ko,
        Self::ZhCn,
        Self::Kk,
        Self::Uk,
    ];
    pub fn from_system_tag(tag: &str) -> Self {
        let normalized = tag.replace('_', "-").to_ascii_lowercase();
        let language = normalized.split('-').next().unwrap_or_default();
        match language {
            "ru" => Self::Ru,
            "es" => Self::Es,
            "pt" => Self::Pt,
            "fr" => Self::Fr,
            "de" => Self::De,
            "it" => Self::It,
            "pl" => Self::Pl,
            "tr" => Self::Tr,
            "ja" => Self::Ja,
            "ko" => Self::Ko,
            "kk" => Self::Kk,
            "uk" => Self::Uk,
            "zh" if !normalized
                .split('-')
                .any(|part| matches!(part, "hant" | "tw" | "hk" | "mo")) =>
            {
                Self::ZhCn
            }
            _ => Self::En,
        }
    }
    pub fn index(self) -> i32 {
        self as i32
    }
    pub fn from_index(index: i32) -> Option<Self> {
        Self::ALL.get(usize::try_from(index).ok()?).copied()
    }
    pub fn from_preference(preference: crate::settings::LanguagePreference, system: Self) -> Self {
        use crate::settings::LanguagePreference as P;
        match preference {
            P::System => system,
            P::Ru => Self::Ru,
            P::En => Self::En,
            P::Es => Self::Es,
            P::Pt => Self::Pt,
            P::Fr => Self::Fr,
            P::De => Self::De,
            P::It => Self::It,
            P::Pl => Self::Pl,
            P::Tr => Self::Tr,
            P::Ja => Self::Ja,
            P::Ko => Self::Ko,
            P::ZhCn => Self::ZhCn,
            P::Kk => Self::Kk,
            P::Uk => Self::Uk,
        }
    }
    pub fn autonym(self) -> &'static str {
        match self {
            Self::Ru => "Русский",
            Self::En => "English",
            Self::Es => "Español",
            Self::Pt => "Português",
            Self::Fr => "Français",
            Self::De => "Deutsch",
            Self::It => "Italiano",
            Self::Pl => "Polski",
            Self::Tr => "Türkçe",
            Self::Ja => "日本語",
            Self::Ko => "한국어",
            Self::ZhCn => "简体中文",
            Self::Kk => "Қазақша",
            Self::Uk => "Українська",
        }
    }
    pub fn decimal_comma(self) -> bool {
        matches!(
            self,
            Self::Ru
                | Self::Es
                | Self::Pt
                | Self::Fr
                | Self::De
                | Self::It
                | Self::Pl
                | Self::Tr
                | Self::Kk
                | Self::Uk
        )
    }
    pub fn text<'a>(self, ru: &'a str, en: &'a str) -> &'a str {
        match self {
            Self::Ru => ru,
            Self::En => en,
            _ => super::translations_generated::lookup(self.index() as usize - 2, ru, en)
                .unwrap_or(en),
        }
    }
}
pub fn state_label(state: TransferState, locale: Locale) -> &'static str {
    match state {
        TransferState::Checking => locale.text("Проверка", "Checking"),
        TransferState::Connecting => locale.text("Подключение", "Connecting"),
        TransferState::Downloading => locale.text("Скачивается", "Downloading"),
        TransferState::WaitingForSources => {
            locale.text("Ожидание источников", "Waiting for sources")
        }
        TransferState::Paused => locale.text("Остановлен", "Stopped"),
        TransferState::Seeding => locale.text("Раздаётся", "Seeding"),
        TransferState::Completed => locale.text("Завершено", "Completed"),
        TransferState::Error => locale.text("Ошибка", "Error"),
    }
}
pub fn error_message(code: &ErrorCode, locale: Locale) -> &'static str {
    match code {
        ErrorCode::TooManyFiles => locale.text("Добавляйте не более 64 торрент-файлов за один раз.", "Add at most 64 torrent files at a time."),
        ErrorCode::InvalidMetadata=>locale.text("Не удалось прочитать торрент. Проверьте файл и попробуйте снова.","Cannot read this torrent. Check the file and try again."),
        ErrorCode::UnsupportedFormat=>locale.text("Этот формат торрента пока не поддерживается. Нужен торрент BitTorrent v1.","This torrent format is not supported yet. Use a BitTorrent v1 torrent."),
        ErrorCode::Magnet=>locale.text("Не удалось загрузить данные magnet-ссылки. Проверьте ссылку и доступность источников, затем повторите попытку.","Could not load this magnet link. Check the link and available sources, then try again."),
        ErrorCode::FileConflict=>locale.text("В этой папке уже есть файлы с такими именами. Выберите другое расположение.","Files with these names already exist. Choose another destination."),
        ErrorCode::UnavailableFolder|ErrorCode::MissingVolume=>locale.text("Папка недоступна. Проверьте подключение и доступ, затем повторите попытку.","The folder is unavailable. Check the drive and folder access, then retry."),
        ErrorCode::DiskFull=>locale.text("Недостаточно места на диске. Освободите место и повторите действие.","The disk is full. Free some space and try again."),
        ErrorCode::PermissionDenied=>locale.text("Нет доступа к папке. Проверьте права или выберите другое расположение.","Access to the folder was denied. Check permissions or choose another destination."),
        ErrorCode::CorruptState=>locale.text("Сохранённые данные повреждены или созданы новой версией. Исходный файл сохранён без изменений.","Saved data is damaged or belongs to a newer version. The original file has been preserved."),
        ErrorCode::InvalidRate=>locale.text("Введите скорость от 0,5 Мбит/с или выберите «Без ограничений».","Enter at least 0.5 Mbit/s or choose Unlimited."),
        ErrorCode::LoginUnavailable=>locale.text("Не удалось изменить автозапуск. Проверьте установку приложения и его запись автозапуска. Настройки не сохранены.","Could not change launch at login. Check the installed application and its login entry. Settings were not saved."),
        ErrorCode::TrashRecovery=>locale.text("Не удалось завершить перенос в Корзину. Данные и журнал сохранены; откройте папку восстановления или повторите перенос через меню.", "Could not finish moving files to Trash. Data and the recovery journal are preserved; open the recovery folder or retry Trash from the menu."),
        ErrorCode::Engine=>locale.text("Не удалось выполнить действие. Попробуйте ещё раз.","The action could not be completed. Try again."),
    }
}

pub fn failure_heading(phase: crate::error::FailurePhase, locale: Locale) -> &'static str {
    use crate::error::FailurePhase;
    match phase {
        FailurePhase::Startup => locale.text(
            "Не удалось запустить FlowTorrent.",
            "FlowTorrent could not start.",
        ),
        FailurePhase::Runtime => locale.text(
            "FlowTorrent неожиданно остановился.",
            "FlowTorrent stopped unexpectedly.",
        ),
        FailurePhase::Shutdown => locale.text(
            "Не удалось корректно завершить работу FlowTorrent.",
            "FlowTorrent could not shut down cleanly.",
        ),
    }
}

pub fn settings_recovery_message(
    recovery: &crate::settings::SettingsRecovery,
    locale: Locale,
) -> String {
    use crate::settings::RecoveryStatus;
    let status = match recovery.status {
        RecoveryStatus::Defaults => locale.text("Используются настройки по умолчанию. Исправленные настройки будут сохранены отдельно:", "Default settings are in use. Corrected settings will be saved separately to:"),
        RecoveryStatus::Recovered => locale.text("Используются восстановленные настройки из файла:", "Recovered settings are in use from:"),
        RecoveryStatus::Blocked => locale.text("Файл восстановления также недоступен. Используются настройки по умолчанию. Сохраните копии обоих файлов и переместите повреждённые файлы перед сохранением:", "The recovery file is also unreadable. Default settings are in use. Keep copies and move the damaged files aside before saving:"),
    };
    format!(
        "{}\n{}\n{}\n{}",
        locale.text(
            "Исходный файл настроек недоступен и сохранён без изменений:",
            "The original settings file could not be read and remains unchanged:"
        ),
        recovery.original.display(),
        status,
        recovery.replacement.display()
    )
}
pub fn format_bytes(bytes: u64, locale: Locale) -> String {
    let units = [
        locale.text("Б", "B"),
        locale.text("КБ", "KB"),
        locale.text("МБ", "MB"),
        locale.text("ГБ", "GB"),
        locale.text("ТБ", "TB"),
    ];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    let number = if value < 10.0 && unit > 0 && value.fract() >= 0.05 {
        format!("{value:.1}")
    } else {
        format!("{value:.0}")
    };
    format!(
        "{} {}",
        if locale.decimal_comma() {
            number.replace('.', ",")
        } else {
            number
        },
        units[unit]
    )
}

pub fn format_rate_equivalent(bytes: u32, locale: Locale) -> String {
    let (scale, precision, unit) = if bytes >= 1_000_000 {
        (1_000_000, 6, locale.text("МБ/с", "MB/s"))
    } else {
        (1000, 3, locale.text("КБ/с", "KB/s"))
    };
    let number = format!(
        "{}.{:0width$}",
        bytes / scale,
        bytes % scale,
        width = precision
    )
    .trim_end_matches('0')
    .trim_end_matches('.')
    .to_owned();
    format!(
        "{} {unit}",
        if locale.decimal_comma() {
            number.replace('.', ",")
        } else {
            number
        }
    )
}

pub fn category_label(category: super::view_state::Category, locale: Locale) -> &'static str {
    use super::view_state::Category;
    match category {
        Category::All => locale.text("Все", "All"),
        Category::Incomplete => locale.text("Загрузки", "Downloading"),
        Category::Downloaded => locale.text("Завершённые", "Completed"),
    }
}
