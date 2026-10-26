use crate::ui::i18n::Locale;
use crate::{domain::*, error::AppResult};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionNotice {
    pub id: TorrentId,
    pub name: String,
}
pub trait NotificationSink: Send + Sync + 'static {
    fn set_locale(&self, _locale: Locale) {}
    fn completed(&self, id: TorrentId, name: &str) -> AppResult<()>;
}
pub fn completion_event(
    record: &mut TorrentRecord,
    snapshot: &TorrentSnapshot,
    enabled: bool,
) -> Option<CompletionNotice> {
    if record.completion_notified
        || record.id != snapshot.id
        || !matches!(
            snapshot.state,
            TransferState::Seeding | TransferState::Completed
        )
        || snapshot.checked_downloaded_bytes < snapshot.selected_bytes
    {
        return None;
    }
    record.completion_notified = true;
    enabled.then(|| CompletionNotice {
        id: record.id,
        name: record.name.clone(),
    })
}
pub fn deliver_notice(sink: &dyn NotificationSink, notice: CompletionNotice) {
    if let Err(error) = sink.completed(notice.id, &notice.name) {
        tracing::warn!(%error, "Completion notification unavailable");
    }
}
pub struct NoopNotificationSink;
impl NotificationSink for NoopNotificationSink {
    fn completed(&self, _: TorrentId, _: &str) -> AppResult<()> {
        Ok(())
    }
}
pub struct NativeNotificationSink {
    russian: std::sync::atomic::AtomicBool,
}
impl NativeNotificationSink {
    pub fn new(locale: Locale) -> Self {
        Self {
            russian: std::sync::atomic::AtomicBool::new(locale == Locale::Ru),
        }
    }
}
impl NotificationSink for NativeNotificationSink {
    fn set_locale(&self, locale: Locale) {
        self.russian
            .store(locale == Locale::Ru, std::sync::atomic::Ordering::Relaxed);
    }
    fn completed(&self, id: TorrentId, name: &str) -> AppResult<()> {
        let locale = if self.russian.load(std::sync::atomic::Ordering::Relaxed) {
            Locale::Ru
        } else {
            Locale::En
        };
        let title = locale.text("Загрузка завершена", "Download complete");
        let name: String = name.chars().filter(|c| !c.is_control()).take(240).collect();
        native_completed(id, title, &name)
    }
}
#[cfg(windows)]
fn native_completed(_id: TorrentId, title: &str, name: &str) -> AppResult<()> {
    use windows::{
        Data::Xml::Dom::XmlDocument,
        UI::Notifications::{ToastNotification, ToastNotificationManager},
        core::{HSTRING, h},
    };
    (|| -> windows::core::Result<()> {
        let xml = XmlDocument::new()?;
        xml.LoadXml(h!("<toast><visual><binding template=\"ToastGeneric\"><text/><text/></binding></visual></toast>"))?;
        let text = xml.GetElementsByTagName(h!("text"))?;
        // Native text nodes preserve filenames literally; no markup or launch
        // command is interpolated from torrent metadata.
        text.Item(0)?.AppendChild(&xml.CreateTextNode(&HSTRING::from(title))?)?;
        text.Item(1)?.AppendChild(&xml.CreateTextNode(&HSTRING::from(name))?)?;
        let toast = ToastNotification::CreateToastNotification(&xml)?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(super::APP_ID))?.Show(&toast)
    })().map_err(anyhow::Error::from)?;
    Ok(())
}
#[cfg(target_os = "macos")]
fn native_completed(id: TorrentId, title: &str, name: &str) -> AppResult<()> {
    use block2::RcBlock;
    use objc2::{rc::autoreleasepool, runtime::Bool};
    use objc2_foundation::{NSBundle, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
        UNUserNotificationCenter,
    };
    autoreleasepool(|_| {
        // UNUserNotificationCenter throws on unbundled tools. Require our bundle
        // identity before calling it; development tools use an explicit no-op sink.
        if NSBundle::mainBundle()
            .bundleIdentifier()
            .is_none_or(|id| id.to_string() != super::APP_ID)
        {
            return Err(anyhow::anyhow!(
                "Notifications require the installed FlowTorrent app bundle"
            )
            .into());
        }
        let title = title.to_owned();
        let name = name.to_owned();
        let authorization = RcBlock::new(move |granted: Bool, error: *mut NSError| {
            if !granted.as_bool() || !error.is_null() {
                tracing::warn!("Completion notification permission denied or unavailable");
                return;
            }
            autoreleasepool(|_| {
                let content = UNMutableNotificationContent::new();
                content.setTitle(&NSString::from_str(&title));
                content.setBody(&NSString::from_str(&name));
                let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                    &NSString::from_str(&id.to_string()),
                    &content,
                    None,
                );
                let done = RcBlock::new(|error: *mut NSError| {
                    if !error.is_null() {
                        tracing::warn!("System could not deliver completion notification");
                    }
                });
                UNUserNotificationCenter::currentNotificationCenter()
                    .addNotificationRequest_withCompletionHandler(&request, Some(&done));
            });
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert,
                &authorization,
            );
        Ok(())
    })
}
#[cfg(not(any(windows, target_os = "macos")))]
fn native_completed(_: TorrentId, _: &str, _: &str) -> AppResult<()> {
    Err(anyhow::anyhow!("Notifications are not available on this platform").into())
}
pub struct NotificationDispatcher {
    sender: std::sync::mpsc::SyncSender<CompletionNotice>,
}
impl NotificationDispatcher {
    pub fn new(sink: Arc<dyn NotificationSink>) -> AppResult<Self> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(32);
        std::thread::Builder::new()
            .name("flowtorrent-notifications".into())
            .spawn(move || {
                while let Ok(notice) = receiver.recv() {
                    deliver_notice(&*sink, notice);
                }
            })?;
        Ok(Self { sender })
    }
    pub fn completed(&self, notice: CompletionNotice) {
        if self.sender.try_send(notice).is_err() {
            tracing::warn!("Completion notification queue unavailable");
        }
    }
}
