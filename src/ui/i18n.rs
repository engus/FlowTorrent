use crate::domain::{ErrorCode, TransferState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    Ru,
    En,
}
impl Locale {
    pub fn from_system_tag(tag: &str) -> Self {
        if tag
            .split(['-', '_'])
            .next()
            .is_some_and(|language| language.eq_ignore_ascii_case("ru"))
        {
            Self::Ru
        } else {
            Self::En
        }
    }
    pub fn text<'a>(self, ru: &'a str, en: &'a str) -> &'a str {
        if self == Self::Ru { ru } else { en }
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
        TransferState::Paused => locale.text("Пауза", "Paused"),
        TransferState::Seeding => locale.text("Раздаётся", "Seeding"),
        TransferState::Completed => locale.text("Завершено", "Completed"),
        TransferState::Error => locale.text("Ошибка", "Error"),
    }
}
pub fn error_message(code: &ErrorCode, locale: Locale) -> &'static str {
    match code {
        ErrorCode::InvalidMetadata=>locale.text("Не удалось прочитать торрент. Проверьте файл и попробуйте снова.","Cannot read this torrent. Check the file and try again."),
        ErrorCode::UnsupportedFormat=>locale.text("Этот формат торрента пока не поддерживается. Нужен файл BitTorrent v1.","This torrent format is not supported yet. Use a BitTorrent v1 file."),
        ErrorCode::FileConflict=>locale.text("В этой папке уже есть файлы с такими именами. Выберите другое расположение.","Files with these names already exist. Choose another destination."),
        ErrorCode::UnavailableFolder|ErrorCode::MissingVolume=>locale.text("Папка недоступна. Подключите диск или выберите другое расположение.","The folder is unavailable. Connect the drive or choose another destination."),
        ErrorCode::DiskFull=>locale.text("Недостаточно места на диске. Освободите место и нажмите «Повторить».","The disk is full. Free some space and choose Retry."),
        ErrorCode::PermissionDenied=>locale.text("Нет доступа к папке. Проверьте права или выберите другое расположение.","Access to the folder was denied. Check permissions or choose another destination."),
        ErrorCode::CorruptState=>locale.text("Сохранённые данные повреждены или созданы новой версией. Исходный файл сохранён без изменений.","Saved data is damaged or belongs to a newer version. The original file has been preserved."),
        ErrorCode::InvalidRate=>locale.text("Введите скорость от 0,02 МБ/с или включите «Без ограничения».","Enter at least 0.02 MB/s or choose Unlimited."),
        ErrorCode::Engine=>locale.text("Не удалось выполнить действие. Попробуйте ещё раз.","The action could not be completed. Try again."),
    }
}
pub fn format_bytes(bytes: u64, locale: Locale) -> String {
    let units = if locale == Locale::Ru {
        ["Б", "КБ", "МБ", "ГБ", "ТБ"]
    } else {
        ["B", "KB", "MB", "GB", "TB"]
    };
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
        if locale == Locale::Ru {
            number.replace('.', ",")
        } else {
            number
        },
        units[unit]
    )
}
