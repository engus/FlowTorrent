use super::i18n::Locale;

use super::components::AboutCopy;

fn destination(index: i32) -> Option<&'static str> {
    match index {
        0 => Some("https://github.com/engus/FlowTorrent"),
        2 => Some("https://golota.me"),
        _ => None,
    }
}
pub fn open_link(
    index: i32,
    launch: impl FnOnce(&str) -> std::io::Result<()>,
) -> Option<std::io::Result<()>> {
    destination(index).map(launch)
}
pub fn link_error(locale: Locale, index: i32) -> String {
    format!(
        "{} {}",
        locale.text(
            "Не удалось открыть браузер. Откройте ссылку вручную:",
            "Could not open the browser. Open this link manually:"
        ),
        destination(index).unwrap_or_default()
    )
}
pub fn copy(locale: Locale) -> AboutCopy {
    AboutCopy {
        title: locale.text("О программе FlowTorrent", "About FlowTorrent").into(),
        description: locale.text("Простой бесплатный BitTorrent-клиент для Windows и macOS. Добавляйте .torrent-файлы и magnet-ссылки, выбирайте нужные файлы и управляйте загрузками без рекламы.", "A simple, free BitTorrent client for Windows and macOS. Add .torrent files and magnet links, choose the files you need and manage downloads without ads.").into(),
        version_label: locale.text("Версия", "Version").into(),
        author: locale.text("Автор: Yevgeniy Golota · engus", "By Yevgeniy Golota · engus").into(),
        credits: locale.text("FlowTorrent — MIT · Движок librqbit — Apache-2.0\nSlint — Royalty-free 2.0 · Lucide — ISC / MIT · Nunito — OFL-1.1", "FlowTorrent — MIT · librqbit engine — Apache-2.0\nSlint — Royalty-free 2.0 · Lucide — ISC / MIT · Nunito — OFL-1.1").into(),
        support_note: locale.text("Ссылка на поддержку появится позже. Все функции доступны бесплатно.", "The support link is coming later. Every feature is available for free.").into(),
        project_label: locale.text("Проект на GitHub", "Project on GitHub").into(),
        support_label: locale.text("Поддержать автора", "Support the author").into(),
        support_available: false,
        author_site_label: locale.text("Сайт автора: golota.me", "Author website: golota.me").into(),
        close_label: locale.text("Закрыть", "Close").into(),
        help_label: locale.text("Справка", "Help").into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_open_project_and_author_but_never_unconfigured_support() {
        assert!(!copy(Locale::En).support_available);
        let mut opened = Vec::new();
        for index in [0, 2] {
            open_link(index, |url| {
                opened.push(url.to_owned());
                Ok(())
            })
            .unwrap()
            .unwrap();
        }
        assert_eq!(
            opened,
            ["https://github.com/engus/FlowTorrent", "https://golota.me"]
        );
        for index in [-1, 1, 3, i32::MAX] {
            assert!(open_link(index, |_| panic!("unknown link launched a browser")).is_none());
        }
    }

    #[test]
    fn browser_failure_is_reported_without_hiding_the_destination() {
        let error = open_link(2, |_| Err(std::io::Error::other("no browser")))
            .unwrap()
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        for locale in [Locale::Ru, Locale::En] {
            assert!(link_error(locale, 2).contains("https://golota.me"));
        }
    }
}
