# FlowTorrent 1.0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Выпустить небольшой надёжный FlowTorrent 1.0 для Windows и macOS: открыть `.torrent`, скачать выбранные файлы и продолжать раздачу в фоне.

**Architecture:** Один Rust-пакет с библиотекой приложения и desktop-бинарником. Slint работает в главном потоке, librqbit — в Tokio; между ними передаются команды и снимки состояния. Общая логика изолирована от системных диалогов, трея, Finder и установщика.

**Tech Stack:** Rust edition 2024, Rust 1.92.0 как начальная фиксированная версия toolchain; librqbit 9.0.1; Slint/slint-build 1.18.1 со встроенным SystemTrayIcon; Tokio, serde/serde_json, thiserror, tracing, async-trait, directories. Системные адаптеры: rfd 0.17.2, notify-rust 4.18.2, single-instance 0.3.3, interprocess 2.4.6; platform-specific Windows/AppKit API при необходимости. Все разрешённые версии фиксируются в Cargo.lock после первой успешной сборки. Версия toolchain повышается отдельным изменением только при подтверждённом требовании зависимости.

**Spec:** [FlowTorrent 1.0 specification](../../spec-1.0.md). Читать перед выполнением плана; при расхождении поведение задаёт спецификация. План не является свидетельством выполненных проверок.

## Global Constraints

- Целевая матрица выпуска: Windows 10 22H2 и Windows 11 на x64; macOS 14 и новее на Apple Silicon и Intel.
- Rust + librqbit + Slint, общий интерфейс. Собственный код и документация — MIT; Slint — Royalty-free с предусмотренной атрибуцией. LICENSE автора не заменять.
- Только открытие локальных `.torrent`. BitTorrent v1 обязателен; гибридные торренты используют v1; pure v2 не входит в обязательную поддержку.
- Основной сценарий: открыть → проверить папку/состав → «Скачать» → закрыть окно → получить уведомление → продолжать раздачу.
- По умолчанию выбраны все пользовательские файлы; состав свёрнут. Выбор после начала загрузки не изменяется.
- Папка сохранения — системная папка «Загрузки» текущего пользователя. Уведомления включены. Оба общих лимита выключены и находятся в свёрнутом разделе «Скорость».
- 1 МБ/с означает 1 000 000 байт/с; минимальный активный лимит — 0,02 МБ/с (20 000 байт/с), согласовано после проверки librqbit в задаче 1. Изменение папки влияет только на новые торренты.
- Тема системная. Русский для русской локали, английский для остальных; отдельных настроек темы/языка нет.
- Закрытие окна сохраняет фоновую работу. «Выйти» и Cmd+Q завершают приложение. После скачивания раздача продолжается до паузы, удаления записи или выхода.
- Список упорядочен по добавлению, новые записи сверху; идентификатор — infohash. Удаление записи сохраняет скачанные файлы.
- Паузы переживают перезапуск. Повторное добавление не создаёт вторую загрузку. Чужие существующие файлы не перезаписываются.
- Публичный релиз требует проверок обеих ОС, установщиков и подписей. Сборки без подписи остаются тестовыми.
- Автообновление, автозапуск, magnet, RSS, поиск, проигрыватель, сложные очереди и сетевые настройки в UI отсутствуют.

## Review Focus

1. Два файла с разными именами и одинаковым infohash, открытые почти одновременно: одна запись и один набор файлов; тест в задаче 3.
2. Сбой во время паузы или удаления: подтверждённое действие переживает перезапуск; удалённый торрент не возвращается; тест в задаче 5.
3. Отключённый внешний диск и ссылки в пути: запись не уходит на другой диск и не выходит за целевую папку; тест в задаче 3 и проверка восстановления в задаче 5.
4. Открытие `.torrent` через Finder при скрытом окне и повторный запуск Windows: тот же процесс получает файл; проверка на реальных ОС в задаче 7.
5. Повторный запуск после завершения: уведомление не приходит повторно, ручная пауза остаётся; тест в задачах 5 и 8.

## Порядок и контрольные результаты

| Этап | Задачи | Проверяемый результат |
| --- | --- | --- |
| Техническая основа | 1 | Окно и настоящая загрузка на обеих ОС, ранняя проверка системных событий |
| Работающий основной сценарий | 2–5 | Безопасное добавление, управление, раздача и восстановление |
| Интерфейс и фоновая работа | 6–8 | Три части UI, открытие из ОС, трей/меню, уведомления |
| Кандидат в релиз | 9–10 | Проверки надёжности, ресурсы, установщики, подписи |

Начальный ориентир — 6–8 полных рабочих недель с резервом. Это не обещание даты: после задачи 1 оценку пересчитать по реальным результатам. Публикация установщиков остаётся отдельным финальным действием после проверки кандидата.

## Структура файлов

Один пакет `flowtorrent`, бинарник `flowtorrent`, библиотека `flowtorrent`. Не создавать workspace из нескольких пакетов до появления подтверждённой необходимости.

| Файлы | Ответственность |
| --- | --- |
| Cargo.toml, Cargo.lock, rust-toolchain.toml, build.rs | Сборка Rust и компиляция Slint |
| src/main.rs, src/lib.rs | Desktop entry point и доступные интеграционным тестам модули |
| src/domain.rs, src/error.rs | Типы, состояния, ошибки с локализуемыми кодами |
| src/settings.rs, src/rate.rs, src/paths.rs, src/persistence.rs | Четыре настройки, единицы скорости, папки приложения и атомарная запись JSON |
| src/engine/mod.rs, src/engine/librqbit.rs | Граница движка и его реализация |
| src/app/mod.rs, src/app/commands.rs, src/app/preview.rs | Владелец реестра, обработка команд, очередь подтверждения файлов |
| src/app/storage.rs, src/app/restore.rs | Устойчивое состояние и восстановление |
| src/app/targets.rs | Проверки мест записи и владение файлами |
| src/ui/mod.rs, src/ui/models.rs, src/ui/i18n.rs | Связь Slint с ядром, модели отображения и строки |
| ui/main.slint, ui/add-torrent.slint, ui/settings.slint, ui/theme.slint, ui/tray.slint | Главное окно, добавление, настройки, две системные темы и встроенный фоновый значок |
| src/platform/mod.rs, src/platform/windows.rs, src/platform/macos.rs | Контракт системной интеграции и реализация для ОС |
| src/platform/instance.rs, src/platform/notifications.rs | Один экземпляр, доставка файлов и уведомления |
| tests/support/mod.rs, tests/fixtures/* | Контролируемая локальная раздача и небольшие синтетические данные |
| tests/engine_smoke.rs, tests/engine_contract.rs | Первичная проверка и контракт движка |
| tests/settings.rs, tests/add_torrent.rs, tests/targets.rs | Настройки, подготовка добавления, защита файлов |
| tests/session.rs, tests/restore.rs, tests/crash_recovery.rs | Команды, восстановление, сбои |
| tests/ui_models.rs, tests/notifications.rs, tests/instance.rs | Модели UI и границы системных событий |
| .github/workflows/ci.yml, .github/workflows/package.yml | Проверки, сборка и упаковка без автоматической публикации |
| packaging/windows/*, packaging/macos/* | Установщик и приложение Mac |
| docs/engineering-validation.md, docs/release-checklist.md, docs/performance.md | Фактические результаты проверки, выпуск и измерения |

## Общие интерфейсы

Контракты ниже — собственный API FlowTorrent, не обещание одноимённых методов librqbit. Его вызовы остаются внутри адаптера. Ненужные публичные API и абстракции плагинов не добавлять.

В `src/domain.rs`: `TorrentId([u8; 20])`, `FileId(usize)`, `DesiredState::{Active, Paused}`, `TransferState::{Checking, Connecting, Downloading, WaitingForSources, Paused, Seeding, Completed, Error}`. `TorrentId` сериализуется как 40 шестнадцатеричных символов.

`TransferLimits { download_bps: Option<NonZeroU32>, upload_bps: Option<NonZeroU32> }`; `Settings { download_dir: PathBuf, notify_on_complete: bool, limits: TransferLimits }`. `TorrentFile { id: FileId, relative_path: PathBuf, length: u64, is_padding: bool }`; `TorrentPreview { id: TorrentId, name: String, files: Vec<TorrentFile>, metadata: Vec<u8>, multi_file: bool }`.

`AddRequest { id: TorrentId, base_dir: PathBuf, selected_files: Vec<FileId> }` ссылается на подготовленный в ядре preview; UI не может подменять метаданные. `TorrentRecord` хранит id, название, порядковый номер добавления, путь к сохранённым метаданным, base_dir, выбранные файлы, desired state и `completion_notified: bool`.

`TorrentSnapshot` содержит id, название, выбранный объём, проверенный скачанный объём, скорости в байт/с, TransferState и необязательный `UserError`. `AppSnapshot` содержит упорядоченные строки, текущие Settings и видимость окна. `UserError { code: ErrorCode, detail: Option<String> }`; техническая detail не заменяет понятную пользователю строку.

В `src/error.rs`: `AppError` и `type AppResult<T> = Result<T, AppError>`. Ошибки различают некорректные метаданные, неподдерживаемый формат, конфликт файлов, недоступную папку, заполненный диск, права, ошибку движка и повреждённое сохранение.

`TorrentEngine: Send + Sync` через async-trait: `inspect(&self, path: &Path) -> AppResult<TorrentPreview>`, `add(&mut self, preview: &TorrentPreview, request: &AddRequest, mode: AddMode) -> AppResult<TorrentId>`, `pause(&mut self, id: TorrentId) -> AppResult<()>`, `resume(&mut self, id: TorrentId) -> AppResult<()>`, `forget(&mut self, id: TorrentId) -> AppResult<()>`, `snapshot(&self) -> AppResult<Vec<TorrentSnapshot>>`, `set_limits(&mut self, limits: TransferLimits) -> AppResult<()>`, `shutdown(&mut self) -> AppResult<()>`. Все перечисленные методы async. `AddMode::{New, RestorePaused}`; только восстановление доверенной собственной записи может разрешать работу с уже существующими файлами.

`AppCommand::{OpenTorrent(PathBuf), ConfirmAdd(AddRequest), CancelPreview, Pause(TorrentId), Resume(TorrentId), Forget(TorrentId), Retry(TorrentId), UpdateSettings(Settings), SetVisible(bool), Shutdown}`. `AppEvent::{PreviewReady(TorrentPreview), Snapshot(AppSnapshot), FocusTorrent(TorrentId), Error(UserError), Completion(TorrentId), ShutdownReady}`. Команды идут через bounded Tokio mpsc, снимки — через watch; между потоками не передаются Slint handles.

## Задача 1 — Подтвердить сборку и связку движка с интерфейсом

**Files:** создать Cargo.toml, Cargo.lock, rust-toolchain.toml, build.rs, src/main.rs, src/lib.rs, src/engine/mod.rs, src/engine/librqbit.rs, ui/main.slint, tests/support/mod.rs, tests/engine_smoke.rs, .github/workflows/ci.yml, docs/engineering-validation.md.

**Interfaces:** потребляет спецификацию; создаёт собираемый пакет и начальную реализацию TorrentEngine. В этой задаче допустимы только необходимые доменные типы из контракта выше; остальное добавляет задача 2.

- [x] Установить Rust toolchain и необходимые Windows C++ build tools, если они отсутствуют; зафиксировать версии в отчёте. Настройка окружения входит в этот этап, не в написание плана.
- [x] Создать минимальный пакет. Для Slint отключить default features, включить `std`, `backend-winit`, `renderer-software`, `compat-1-2`, `system-tray`, `accessibility`. Не подключать Qt или WebView. Tokio — multi-thread runtime вне UI. Сначала проверить CPU renderer; другой renderer подключать только по результату проверки.
- [x] Написать `controlled_swarm_downloads_known_bytes`: локальный seeder отдаёт синтетические данные, downloader сохраняет их в tempdir; проверка `assert_eq!(std::fs::read(destination)?, fixture_bytes);`, внешний интернет не требуется. Сначала тест должен падать из-за отсутствующей реализации.
- [x] Реализовать минимальный адаптер, затем выполнить `cargo test --test engine_smoke -- --nocapture` и `cargo run --release`. Проверить живое обновление прогресса без блокировки окна.
- [x] Дополнить smoke-проверку выбором одного файла, паузой/продолжением, восстановлением из существующих проверяемых файлов, ограничением скорости и private-торрентом. Для private-раздачи перехватить обращения discovery: private infohash не должен уходить в DHT/PEX/LSD.
- [ ] Проверить жизненный цикл Winit/Slint, скрытие последнего окна и доставку открытия файла Finder при уже запущенном процессе. Не откладывать выявление ограничений AppKit delegate до задачи 7. Рабочий подход записать в docs/engineering-validation.md; полный пользовательский сценарий реализует задача 7.
- [ ] В CI запустить build/test на `windows-2022` / x86_64-pc-windows-msvc, `macos-15` / aarch64-apple-darwin и `macos-15-intel` / x86_64-apple-darwin. Для Mac выставить MACOSX_DEPLOYMENT_TARGET=14.0. Публичные сетевые торренты не делать условием unit/integration CI.
- [ ] Проверить старт приложения на реальном Mac. Успешную сборку в CI не записывать как проверку Finder, Dock, меню или Retina. Если Mac пока недоступен, явно сохранить непроверенные пункты в отчёте; не объявлять их выполненными и не обещать релиз.
- [x] Запустить `cargo fmt --all -- --check`, `cargo check --locked --all-targets` и тесты. Зафиксировать результат и начальную память release-сборки. При блокирующей несовместимости обновить решение до роста UI, а не писать собственный движок.
- [ ] Commit: `feat: establish cross-platform Slint and librqbit foundation`.

**Готово:** получены окно и настоящая загрузка на Windows/Mac; известны ограничения сохранения, private-раздач, записи файлов и платформенных событий. Тестовый harness остаётся инструментом проверок, не новой функцией продукта.

## Задача 2 — Типы и четыре настройки

**Files:** создать src/domain.rs, src/error.rs, src/settings.rs, src/rate.rs, src/paths.rs, src/persistence.rs, tests/settings.rs; уточнить src/engine/mod.rs и Cargo.toml.

**Interfaces:** потребляет контракт доменных типов. Создаёт `Settings::defaults(download_dir: PathBuf) -> Settings`, `parse_rate_mbps(text: &str, unlimited: bool) -> AppResult<Option<NonZeroU32>>`, `SettingsStore::load(&self) -> AppResult<Settings>`, `SettingsStore::save(&self, settings: &Settings) -> AppResult<()>`, `AppPaths::discover() -> AppResult<AppPaths>` и `atomic_write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()>`. Определить TorrentRecord и `Manifest { schema_version: u32, next_position: u64, torrents: Vec<TorrentRecord> }` в domain.rs для использования следующими этапами.

- [x] Написать тесты defaults и преобразования единиц: папка из переданного системного пути, notify=true, оба лимита=None; `assert_eq!(parse_rate_mbps("1.5", false)?.unwrap().get(), 1_500_000);`, аналогично для `1,5`; unlimited=true → None; 0,02 МБ/с принимается; меньше 0,02, ноль, отрицательное, NaN и переполнение → ошибка без изменения сохранённого значения.
- [x] Выполнить `cargo test --test settings`; убедиться, что недостающая логика делает тесты красными.
- [x] Реализовать типы и конвертацию без округления до нуля. Использовать стандартные пользовательские каталоги через directories; не собирать путь «Загрузки» из имени папки вручную. Ошибка обнаружения или отсутствие write access не создаёт скрытую альтернативную папку.
- [x] Реализовать JSON schema_version=1 и устойчивое сохранение: temp-файл в том же каталоге, flush/sync и атомарная замена с сохранением предыдущей исправной копии. На Windows не удалять старый файл перед rename. Для последующих записей сессии переиспользовать этот механизм.
- [x] Проверить round-trip, сохранение при прерванной записи и повреждённый JSON: ошибка понятна, исходные байты сохраняются, пустая конфигурация поверх не записывается. Выполнить `cargo test --test settings` → PASS.
- [x] Commit: `feat: add minimal settings and shared domain types`.

## Задача 3 — Подготовка торрента и защита существующих файлов

**Files:** создать src/app/mod.rs, src/app/commands.rs, src/app/preview.rs, src/app/targets.rs, tests/add_torrent.rs, tests/targets.rs; дополнить tests/fixtures и адаптер.

**Interfaces:** потребляет TorrentEngine::inspect и AppPaths. Создаёт `PreviewQueue::enqueue(path: PathBuf)`, `PreviewQueue::cancel_current()`, `prepare_add(preview: &TorrentPreview, request: &AddRequest) -> AppResult<PreparedAdd>`. `PreparedAdd` содержит проверенное конечное расположение и служит единственным входом к созданию новой загрузки. `Arc<PreparedAdd>::confirm()` захватывает файлы через create_new и возвращает AddRequest с закрытым набором файловых дескрипторов. Controller сначала подтверждает владение файлами, затем сохраняет metainfo и manifest и только потом передаёт запрос движку. RestorePaused разрешён только для записей manifest; проверка записи реализуется этапом 5.

- [x] Написать тесты `cancel_does_not_create_payload`, `empty_selection_is_rejected`, `same_infohash_is_not_added_twice_concurrently`, `single_and_multi_file_destinations_match_preview` и `existing_payload_is_not_overwritten`. Проверка пустого выбора: `assert!(prepare_add(&preview, &empty_request).is_err());`. Для одновременного добавления использовать одинаковые метаданные под разными именами.
- [x] Выполнить `cargo test --test add_torrent --test targets` и увидеть красный результат до реализации.
- [x] Реализовать очередь preview и проверку выбора. Отсеять padding из пользовательского списка, сохранив исходные file indices движка. Резервировать infohash при подтверждении, чтобы два параллельных подтверждения не прошли проверку как новые.
- [x] Проверить метаданные до записи: абсолютные пути, `..`, Windows drive/UNC paths, конфликтующие после нормализации имена, symlink/reparse point в целевых родителях → отказ. `targets_rejects_escape_and_links` проверяет, что файл вне target не изменился. Отдельно проверить подмену пути между preview и началом записи; одних предварительных строковых проверок недостаточно.
- [x] Реализовать подтверждение итогового расположения и безопасное открытие/создание файлов без clobber для New. Использовать проверенную защиту engine storage; если она недостаточна, ограничить операции адаптером StorageFactory. Для RestorePaused допускать только сохранённую собственную запись и повторную проверку содержимого.
- [x] Проверить кириллицу, длинные имена, недоступный внешний диск и неподдерживаемый pure-v2: понятная ошибка, без записи в другое место. Повторить оба test targets → PASS.
- [x] Commit: `feat: add safe torrent preview and confirmation`.

## Задача 4 — Загрузка, раздача и команды

**Files:** дополнить src/app/mod.rs, src/app/commands.rs, src/engine/librqbit.rs; создать src/app/storage.rs с контрактом хранения, tests/session.rs и tests/engine_contract.rs.

**Interfaces:** потребляет PreparedAdd, TorrentEngine и SettingsStore. Создаёт `SessionRepository: Send + Sync` с `load(&self) -> AppResult<Manifest>` и `save(&self, manifest: &Manifest) -> AppResult<()>`, а также `AppController::spawn(engine: Box<dyn TorrentEngine>, store: Arc<dyn SessionRepository>, settings_store: Arc<SettingsStore>, paths: AppPaths) -> AppHandle`; `AppHandle { commands: mpsc::Sender<AppCommand>, snapshots: watch::Receiver<AppSnapshot>, events: mpsc::Receiver<AppEvent> }`. In-memory repository находится только в tests/support для проверки controller. Продуктовый store реализует задача 5; окончательное подключение controller к main выполняется в задаче 6.

- [ ] Написать тесты состояний: подтверждение добавляет одну строку; выборка прогресса относится только к selected_files; пауза прекращает обе передачи; completion → Seeding; пауза после completion проверяется как `assert_eq!(snapshot.state, TransferState::Completed);`; ожидание источников не превращается в Error.
- [ ] Выполнить `cargo test --test session --test engine_contract` и зафиксировать ожидаемые FAIL до реализации.
- [ ] Реализовать одного владельца реестра с bounded mpsc, coalescing снимков и порядком newest-first. Не создавать отдельный планировщик очереди. При гонке команды со снимком состояние UI меняется только после результата операции; устаревший снимок не отменяет ручную паузу.
- [ ] Применить общие лимиты через API сессии librqbit. Тест `two_torrents_share_global_limits` использует два downloader jobs и локальный seeder: лимит делится между ними, а не умножается на число торрентов. Измерять полезные данные после прогрева, отдельно от протокольных накладных расходов.
- [ ] Добавить контролируемые ошибки одного торрента и Retry: остальные строки продолжают работать. Проверить `failed_add_keeps_an_explainable_error` и `remove_keeps_payload_and_stops_transfer`.
- [ ] Повторить тесты → PASS, затем локальную загрузку через минимальное окно из задачи 1.
- [ ] Commit: `feat: implement torrent lifecycle and global speed limits`.

## Задача 5 — Устойчивая сессия и восстановление

**Files:** дополнить src/app/storage.rs; создать src/app/restore.rs, tests/restore.rs, tests/crash_recovery.rs; дополнить controller, settings storage и адаптер.

**Interfaces:** `SessionStore` реализует SessionRepository из задачи 4 для Manifest из domain.rs. Создаёт `restore_session(engine: &mut dyn TorrentEngine, store: &SessionStore) -> AppResult<Manifest>` как async-функцию. Реальные файловые операции store выполняются через spawn_blocking, сохраняя последовательность команд controller.

- [ ] Написать `restart_restores_paths_selection_and_pause`, `original_torrent_file_is_not_required`, `confirmed_remove_is_not_resurrected`, `corrupt_manifest_is_preserved`, `completed_notification_stays_acknowledged`. После подтверждённой паузы: `assert_eq!(store.load()?.torrents[0].desired_state, DesiredState::Paused);`. Запустить `cargo test --test restore --test crash_recovery` → ожидаемый FAIL.
- [ ] Сделать manifest приложения единственным источником списка и желаемых состояний. Сохранять исходные метаданные под infohash в app data. Встроенное автоматическое восстановление списка librqbit отключить, чтобы две базы не воскрешали удалённые записи.
- [ ] Восстанавливать доверенные записи через RestorePaused, проверять существующие части, затем возобновлять только desired=Active. Для 1.0 корректная повторная проверка существующих данных допустима. Fastresume не добавлять ценой второго источника истины; ускорение отдельно после доказанной корректности.
- [ ] Сохранять подтверждённое намерение до успешного ответа UI. Пауза считается показанной как успешная после подтверждения движка; при ошибке отображается ошибка, а не выдуманная пауза. Удаление сначала останавливает передачу, затем надёжно исключает запись из manifest; оставшиеся метаданные движка не восстанавливают её.
- [ ] Реализовать тот же atomic write, что у SettingsStore, и понятное состояние при повреждении. Обновление структуры делается по schema_version; неизвестная будущая версия не затирается.
- [ ] В подпроцессе завершать приложение в точках до/после устойчивой записи Add/Pause/Forget, затем повторно запускать. Проверить сохранённые намерения, отсутствие посторонней перезаписи и повторное использование проверенных частей. Пропавший диск остаётся ошибкой своей записи без перенаправления пути.
- [ ] Повторить тесты → PASS, проверить обычный перезапуск и принудительное завершение desktop-сборки.
- [ ] Commit: `feat: persist sessions and recover interrupted downloads`.

## Задача 6 — Закончить три части интерфейса Slint

**Files:** создать ui/add-torrent.slint, ui/settings.slint, ui/theme.slint, src/ui/mod.rs, src/ui/models.rs, src/ui/i18n.rs, tests/ui_models.rs; дополнить ui/main.slint и main.

**Interfaces:** потребляет AppHandle/AppSnapshot. Создаёт `UiController::new(handle: AppHandle) -> AppResult<UiController>`, `UiController::window(&self) -> &slint::Window`, `to_row(snapshot: &TorrentSnapshot, locale: Locale) -> TorrentRowModel`, `Locale::{Ru, En}` и `Locale::from_system_tag(tag: &str) -> Locale`. `TorrentRowModel` — обычная Rust-модель отображения до преобразования в Slint model. PlatformShell в задаче 7 подключается к window().

- [ ] Написать тесты точных строк состояний RU/EN, Completed после паузы раздачи, selected progress, newest-first, ограничения нулевого выбора; `assert_eq!(Locale::from_system_tag("ru-RU"), Locale::Ru);`, аналогично для `ru`, для остальных локалей ожидается En. Проверить, что путь или название файла не переводятся. `cargo test --test ui_models` сначала FAIL.
- [ ] Реализовать главное окно, диалог добавления и одну страницу настроек. Состав и «Скорость» свёрнуты по умолчанию; глобальных настроек ровно четыре. Без боковых категорий и новых функций из исключённого объёма.
- [ ] Привязать callback к командам через weak handles и invoke_from_event_loop. Сохранять выделение и scroll при обновлении строк. Каждые 500 мс обновлять только изменившиеся данные; при скрытом окне UI timer не работает.
- [ ] Реализовать ошибки рядом с действием, подтверждение «Убрать из списка» с текстом о сохранении файлов, полный текст длинного имени по наведению/доступному раскрытию, фокус и Ctrl+O/Cmd+O. Папку после выбора одного торрента не записывать как новую глобальную настройку.
- [ ] Проверить обе системные темы, RU/EN, keyboard-only путь добавления и доступность основных кнопок. На Windows проверить 100/125/150/200%, на Mac обычный экран и Retina. Записать результаты, не заменять их скриншотом из другого проекта.
- [ ] Выполнить `cargo test --test ui_models` → PASS и `cargo check --locked --all-targets`; затем ручной путь с реальной загрузкой и перезапуском.
- [ ] Commit: `feat: build minimal Slint download interface`.

## Задача 7 — Открытие из ОС, один экземпляр и фоновое окно

**Files:** создать src/platform/mod.rs, src/platform/windows.rs, src/platform/macos.rs, src/platform/instance.rs, ui/tray.slint, tests/instance.rs; дополнить main и UiController.

**Interfaces:** `PlatformEvent::{OpenTorrentPaths(Vec<PathBuf>), Reopen, Quit, ThemeChanged(bool)}`. `InstanceGuard::acquire(paths: &AppPaths) -> AppResult<InstanceRole>`; `InstanceRole::{Primary(InstanceGuard), Secondary}`. `forward_open(paths: &[PathBuf]) -> AppResult<()>`. `PlatformShell::attach(window: &slint::Window, events: std::sync::mpsc::Sender<PlatformEvent>) -> AppResult<PlatformShell>`, `PlatformShell::show_main(&self) -> AppResult<()>`, `PlatformShell::hide_main(&self) -> AppResult<()>`, `PlatformShell::finish_exit(&self) -> AppResult<()>`. Bool в ThemeChanged означает dark=true. Мост системных событий передаёт команды в AppHandle; полный bounded queue обрабатывается без потери Quit и без ожидания сети в UI thread.

- [ ] Написать tests/instance.rs: первый процесс удерживает guard, второй передаёт два пути с пробелами/кириллицей, guard не освобождается до выхода, сообщение приходит один раз и в исходном порядке: `assert_eq!(received_paths, vec![first_path, second_path]);`. Проверить гонку запуска до готовности listener: bounded retry, без второго engine session. Запустить `cargo test --test instance` → FAIL.
- [ ] Использовать single-instance как process guard, interprocess для локальной доставки. Передавать структурированные пути без shell и command-string parsing; endpoint доступен текущему пользователю. Проверять размер сообщения и число путей; некорректное сообщение не завершает primary.
- [ ] На Windows принимать argv на холодном старте и IPC на тёплом. На Mac подключить проверенный в задаче 1 адаптер событий `openFiles`/reopen, сохранив работу Slint/Winit и его delegate. Только проверять argc недостаточно.
- [ ] Создавать встроенный Slint SystemTrayIcon/menu на главном потоке; не добавлять отдельный tray-icon runtime. CloseRequested скрывает окно и оставляет event loop живым; использовать run_event_loop_until_quit для явного управления завершением. Quit начинает сохранение/shutdown и вызывает quit_event_loop только после ShutdownReady. Cmd+Q и Dock reopen следуют этому же контракту.
- [ ] Один раз объяснить фоновую работу, сохранив факт показа. Проверить открытие из трея и меню Mac, закрытие последнего окна, повторное появление, перенос диалогов добавления на передний план.
- [ ] Выполнить `cargo test --test instance` → PASS. На обеих ОС проверить холодный/тёплый запуск двойным щелчком, скрытое окно и последовательное подтверждение нескольких файлов. По Ctrl+C в терминале и обычному выходу нет зависшего фонового процесса.
- [ ] Commit: `feat: integrate native file opening and background lifecycle`.

## Задача 8 — Уведомления и пользовательские ошибки

**Files:** создать src/platform/notifications.rs, tests/notifications.rs; дополнить src/ui/i18n.rs, src/app/mod.rs, restore и platform adapters.

**Interfaces:** `NotificationSink::completed(id: TorrentId, name: &str) -> AppResult<()>`; `completion_event(record: &mut TorrentRecord, snapshot: &TorrentSnapshot, enabled: bool) -> Option<CompletionNotice>`. UI получает ErrorCode с локализованным текстом и Retry только для устранимой ошибки.

- [ ] Написать `one_completion_notice_across_restart`, `disabled_or_denied_notifications_do_not_break_downloads`, `selected_files_completion_is_enough`, `waiting_for_sources_is_not_error`. Для записи с completion_notified=true проверка `assert!(completion_event(&mut record, &completed_snapshot, true).is_none());`. При выключенных уведомлениях событие completion учитывается, но при последующем включении старые завершения не рассылаются. Запустить `cargo test --test notifications` → FAIL.
- [ ] Сохранять completion_notified до отправки системного уведомления. Выбрать at-most-once при сбое между сохранением и отправкой: лучше пропущенное уведомление, чем повторные сообщения при каждом запуске. Скачивание и раздача от успешности уведомления не зависят.
- [ ] Подключить notify-rust/системные API. Ошибка разрешений и отсутствие notification service не переводят торрент в Error. Уведомление не запускает автоматически скачанный файл.
- [ ] Добавить понятные тексты для полного диска, отсутствующего тома, прав, повреждённого `.torrent`, неподдерживаемого формата и повреждённого manifest; технические детали направлять в локальный ограниченный лог.
- [ ] Выполнить тесты → PASS. Проверить одно завершение в фоне и сохранение паузы после перезапуска на Windows/Mac; проверить системный отказ в разрешении уведомлений.
- [ ] Commit: `feat: add completion notifications and actionable errors`.

## Задача 9 — Надёжность, ресурсы и кандидат в релиз

**Files:** дополнить tests/engine_contract.rs, tests/crash_recovery.rs, tests/targets.rs; создать docs/performance.md, docs/release-checklist.md и локальный измерительный harness в examples/measure_resources.rs.

**Interfaces:** потребляет готовый основной сценарий и тестовый seeder. Создаёт воспроизводимый протокол проверки ресурсов и отчёт со сборкой/ОС/архитектурой/рендерером/метрикой, а не универсальное обещание RAM.

- [ ] Добавить недостающие проверки спецификации: сеть пропала/вернулась, внешнее хранилище отключено, ENOSPC/permission failure, повторная проверка повреждённого куска. Для теста отсутствия места использовать управляемый storage failure, не заполнять системный диск.
- [ ] Выполнить `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked --all-targets` и `cargo build --locked --release` на всей матрице → PASS. Нет игнорирования теста для скрытия неисправности.
- [ ] Измерить пустое окно, пять активных торрентов, двадцать записей и скрытое окно; сравнивать одинаковую нагрузку. На Windows записать private bytes и working set процесса, на Mac resident size и доступный physical footprint. GPU/shared cache не выдавать за ту же метрику. Проверить повторные hide/show и отсутствие удержания UI closures.
- [ ] Провести ночную загрузку/раздачу и восстановление после сна/принудительного выхода. Проверить все пять Review Focus сценариев. Проблемы фиксировать отдельными небольшими коммитами с воспроизводящим тестом.
- [ ] Заполнить release checklist фактическими результатами, выделить непроверенные среды. Измерения 50–150 МБ использовать только как первоначальный ориентир; не подменять ими результаты.
- [ ] Commit: `test: validate release reliability and resource usage`.

## Задача 10 — Установщики, лицензии и подготовка выпуска

**Files:** создать packaging/windows/flowtorrent.iss, packaging/macos/Info.plist, packaging/macos/entitlements.plist, scripts/package-windows.ps1, scripts/package-macos.sh, .github/workflows/package.yml, THIRD_PARTY_NOTICES.md и CHANGELOG.md; обновить README.md и release checklist.

**Interfaces:** вход — проверенный release-бинарник и Cargo.lock. Выход — per-user Windows installer EXE и отдельные DMG для aarch64/x86_64 Mac, контрольные суммы и инструкции. Сначала workflow собирает artifacts, публикации GitHub Release по push/tag нет.

- [ ] Упаковать Windows через Inno Setup, Mac через `.app` + `hdiutil`. Версия бинарника, bundle и установщика совпадает. Идентификатор приложения `com.engus.flowtorrent`; display name FlowTorrent. На Mac deployment target=14.0, обе архитектуры пакуются и проверяются отдельно.
- [ ] Зарегистрировать обработчик `.torrent`: Windows ProgID `FlowTorrent.Torrent`, Mac UTI BitTorrent и role Viewer. Не менять default app принудительно: пользователь выбирает его системными средствами. Добавить идентификацию приложения для штатных уведомлений.
- [ ] Собрать уведомления о лицензиях из точного Cargo.lock. Сохранить MIT автора, тексты Apache/MIT/других зависимостей и Slint Royalty-free, а также видимый официальный badge в README и шаблоне страницы скачивания. Не объявлять всю цепочку зависимостей MIT.
- [ ] Подпись Windows и Developer ID/notarization подключить через защищённые credentials окружения/CI. Сертификаты, пароли и токены не попадут в репозиторий или логи. Если credentials недоступны, собрать явно тестовые artifacts и оставить пункт публичного подписанного выпуска невыполненным.
- [ ] Проверить установку на чистой системе, открытие `.torrent`, фоновые загрузки, обновление поверх предыдущей тестовой сборки с сохранением сессии и удаление программы без удаления скачанных данных. Тесты release-сборок выполнять после подписания/упаковки.
- [ ] Обновить README: фактический статус, установка, краткий сценарий, ручное обновление, известные ограничения v1/private и контакт для ошибки. Подготовить CHANGELOG и SHA-256. Сверить каждый из 12 критериев спецификации с записью проверки.
- [ ] Commit: `build: prepare Windows and macOS release packages`.

**Готово:** обе платформы проходят спецификацию, artifacts и заметки готовы к отдельной публикации. Версия 1.0.0 не объявляется готовой по одной успешной локальной сборке.

## Проверка покрытия спецификации

| Раздел спецификации | Реализация и проверка |
| --- | --- |
| 1 — платформы и границы | 1, 9, 10 |
| 2 — основной сценарий | 3, 4, 6, 7, 8 |
| 3 — главное окно и состояния | 4, 6 |
| 4 — файлы, дубликаты, папки | 3, 5, 7 |
| 5 — четыре настройки, темы, язык | 2, 4, 6, 8 |
| 6 — фон, выход, сон, раздача | 4, 5, 7, 9 |
| 7 — восстановление и ошибки | 3, 5, 8, 9 |
| 8 — архитектура и MIT | 1, 2, 4, 5, 6, 10 |
| 9 — совместимость движка | 1, 3, 4, 9 |
| 10 — вид и ресурсы | 6, 9 |
| 11 — критерии выпуска | 9, 10 |
| 12 — исключённые функции | Global Constraints, review каждого изменения |

## Выполнение плана

По умолчанию задачи выполняются последовательно: соседние этапы зависят от общих типов и поведения движка. Перед реализацией пользователь просматривает план и выбирает выполнение в текущей сессии либо отдельными исполнителями с проверкой. Самостоятельная проверка этого документа не требует запуска субагентов.

Рекомендуемый вариант — выполнение в текущей сессии с проверкой после каждого этапа и отдельными коммитами. Публикация release не является автоматическим следствием разрешения делать коммиты. Изменения спецификации и выявленные ограничения записываются в репозиторий вместе с соответствующим решением.

## Источники для реализации

- [librqbit 9.0.1 API](https://docs.rs/librqbit/9.0.1/librqbit/).
- [Session 9.0.1 source](https://github.com/ikatson/rqbit/blob/v9.0.1/crates/librqbit/src/session.rs) — список persistence автоматически восстанавливается при создании сессии; не включать его как независимый реестр.
- [Slint Rust API](https://docs.slint.dev/latest/docs/rust/slint/) и [Cargo features 1.18.1](https://github.com/slint-ui/slint/blob/v1.18.1/api/rs/slint/Cargo.toml).
- [GitHub runner images](https://github.com/actions/runner-images) — доступные архитектуры macOS и Windows.
- [Slint Royalty-free](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md).
