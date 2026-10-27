use super::{
    components::{AddWindow, MainWindow, Translations},
    i18n::Locale,
};
use crate::{error::AppResult, platform::modal::ModalGuard};
use slint::ComponentHandle;
use std::{
    any::Any,
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

type Attach = Rc<dyn Fn(&slint::Window, &slint::Window) -> AppResult<Box<dyn Any>>>;
type Ready = Rc<dyn Fn(&slint::Window, &slint::Window) -> bool>;
#[derive(Clone, PartialEq, Eq)]
enum Flow {
    Magnet,
    Preview(String),
}
struct OpenWindow {
    view: AddWindow,
    lease: Option<Box<dyn Any>>,
    active: Rc<Cell<bool>>,
    flow: Flow,
    waiting: Rc<Cell<bool>>,
    needs_focus: bool,
}
impl Drop for OpenWindow {
    fn drop(&mut self) {
        self.active.set(false);
        self.lease.take();
        let _ = self.view.hide();
    }
}
pub(super) struct AddWindowCoordinator {
    open: Option<OpenWindow>,
    failed: Option<Flow>,
    attach: Attach,
    ready: Ready,
}
impl AddWindowCoordinator {
    fn new() -> Self {
        let mut coordinator = Self::with_attach(Rc::new(|parent, child| {
            Ok(Box::new(ModalGuard::attach(parent, child)?))
        }));
        coordinator.ready = Rc::new(ModalGuard::ready);
        coordinator
    }
    fn with_attach(attach: Attach) -> Self {
        Self {
            open: None,
            failed: None,
            attach,
            ready: Rc::new(|_, _| true),
        }
    }
    pub fn window(&self) -> Option<&AddWindow> {
        self.open.as_ref().map(|open| &open.view)
    }
    pub fn close(&mut self) {
        self.open.take();
        self.failed = None;
    }
    pub fn sync(&mut self, app: &MainWindow) -> AppResult<()> {
        if app.get_stopping()
            || !app.window().is_visible()
            || (!app.get_preview_open() && !app.get_show_magnet())
        {
            self.close();
            app.set_add_window_failed(false);
            return Ok(());
        }
        let flow = if app.get_preview_open() {
            Flow::Preview(app.get_add_preview_id().to_string())
        } else {
            Flow::Magnet
        };
        if self.failed.as_ref() == Some(&flow) {
            return Ok(());
        }
        if self
            .open
            .as_ref()
            .is_some_and(|open| matches!(&open.flow, Flow::Preview(_)) && open.flow != flow)
        {
            self.close();
        }
        let result = self.update(app, flow.clone());
        if result.is_err() {
            self.open.take();
            self.failed = Some(flow);
            app.set_add_window_failed(true);
            let locale = Locale::from_index(app.get_dialog_locale()).unwrap_or(Locale::En);
            let message = locale.text(
                "Не удалось открыть окно добавления. Повторите или отмените добавление.",
                "The add window could not be opened. Retry or cancel the add.",
            );
            app.set_error(message.into());
            app.invoke_native_error(message.into());
        }
        result
    }
    fn update(&mut self, app: &MainWindow, flow: Flow) -> AppResult<()> {
        let fresh = self.open.is_none();
        if fresh {
            let view = AddWindow::new().map_err(anyhow::Error::from)?;
            let active = Rc::new(Cell::new(true));
            let waiting = Rc::new(Cell::new(true));
            connect(&view, app, active.clone(), waiting.clone());
            self.open = Some(OpenWindow {
                view,
                lease: None,
                active,
                flow: flow.clone(),
                waiting,
                needs_focus: true,
            });
        }
        let open = self.open.as_mut().unwrap();
        open.needs_focus |= open.flow != flow;
        open.view.set_busy(true);
        open.view
            .global::<Translations>()
            .set_locale(app.get_dialog_locale());
        open.view.set_dark(app.get_dialog_dark());
        open.view.set_ru(app.get_ru());
        open.view.set_preview_name(app.get_preview_name());
        open.view.set_preview_size(app.get_preview_size());
        open.view.set_preview_folder(app.get_preview_folder());
        open.view
            .set_preview_final_path(app.get_preview_final_path());
        open.view.set_preview_files(app.get_preview_files());
        open.view
            .set_preview_can_confirm(app.get_preview_can_confirm());
        open.view.set_files_expanded(app.get_files_expanded());
        open.view.set_magnet_link(app.get_magnet_link());
        open.view.set_resolving_magnet(app.get_resolving_magnet());
        open.view.set_error(app.get_error());
        open.view.set_preview_id(app.get_add_preview_id());
        open.view.set_preview_open(app.get_preview_open());
        open.flow = flow;
        if fresh {
            open.view.show().map_err(anyhow::Error::from)?;
        }
        if open.lease.is_none() && (self.ready)(app.window(), open.view.window()) {
            #[cfg(target_os = "windows")]
            {
                use slint::winit_030::{
                    WinitWindowAccessor, winit::platform::windows::WindowExtWindows,
                };
                open.view
                    .window()
                    .with_winit_window(|window| window.set_skip_taskbar(true));
            }
            open.lease = Some((self.attach)(app.window(), open.view.window())?);
            open.waiting.set(false);
        }
        open.view.set_busy(app.get_busy() || open.lease.is_none());
        if open.needs_focus && !open.view.get_busy() {
            open.view
                .set_focus_ticket(open.view.get_focus_ticket().wrapping_add(1));
            open.needs_focus = false;
        }
        app.set_add_window_failed(false);
        Ok(())
    }
}

fn current(
    parent: &slint::Weak<MainWindow>,
    child: &slint::Weak<AddWindow>,
    active: &Cell<bool>,
) -> Option<(MainWindow, AddWindow)> {
    if !active.get() {
        return None;
    }
    let parent = parent.upgrade()?;
    let child = child.upgrade()?;
    if parent.get_stopping() || parent.get_busy() || child.get_busy() {
        return None;
    }
    if child.get_preview_open() {
        if !parent.get_preview_open() || child.get_preview_id() != parent.get_add_preview_id() {
            return None;
        }
    } else if !parent.get_show_magnet() || parent.get_preview_open() {
        return None;
    }
    Some((parent, child))
}
fn connect(view: &AddWindow, parent: &MainWindow, active: Rc<Cell<bool>>, waiting: Rc<Cell<bool>>) {
    use slint::winit_030::{EventResult, WinitWindowAccessor};
    let ready_parent = parent.as_weak();
    let token = active.clone();
    view.window().on_winit_window_event(move |_, _| {
        if token.get()
            && waiting.get()
            && let Some(parent) = ready_parent.upgrade()
        {
            parent.invoke_add_window_changed();
        }
        EventResult::Propagate
    });
    view.global::<Translations>().on_lookup(|index, ru, en| {
        Locale::from_index(index)
            .unwrap_or(Locale::En)
            .text(&ru, &en)
            .into()
    });
    macro_rules! forward {
        ($on:ident,$body:expr) => {{
            let parent = parent.as_weak();
            let child = view.as_weak();
            let active = active.clone();
            view.$on(move || {
                if let Some((parent, child)) = current(&parent, &child, &active) {
                    ($body)(&parent, &child);
                }
            });
        }};
    }
    forward!(
        on_confirm_preview,
        |parent: &MainWindow, child: &AddWindow| if child.get_preview_open()
            && parent.get_preview_can_confirm()
        {
            parent.invoke_confirm_preview();
        }
    );
    forward!(
        on_choose_preview_folder,
        |parent: &MainWindow, child: &AddWindow| if child.get_preview_open() {
            parent.invoke_choose_preview_folder();
        }
    );
    forward!(
        on_submit_magnet,
        |parent: &MainWindow, child: &AddWindow| if !child.get_preview_open() {
            parent.invoke_submit_magnet();
        }
    );
    forward!(
        on_magnet_link_edited,
        |parent: &MainWindow, child: &AddWindow| if !child.get_preview_open() {
            parent.set_magnet_link(child.get_magnet_link());
        }
    );
    forward!(
        on_expansion_edited,
        |parent: &MainWindow, child: &AddWindow| parent
            .set_files_expanded(child.get_files_expanded())
    );
    forward!(
        on_cancel,
        |parent: &MainWindow, child: &AddWindow| if child.get_preview_open() {
            parent.invoke_cancel_preview();
        } else {
            parent.invoke_cancel_magnet();
        }
    );
    let weak_parent = parent.as_weak();
    let child = view.as_weak();
    let token = active.clone();
    view.on_toggle_preview_file(move |id, checked| {
        if let Some((parent, child)) = current(&weak_parent, &child, &token)
            && child.get_preview_open()
        {
            parent.invoke_toggle_preview_file(id, checked);
        }
    });
    let child = view.as_weak();
    view.window().on_close_requested(move || {
        if let Some(child) = child.upgrade()
            && !child.get_busy()
        {
            child.invoke_cancel();
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
}

pub(super) fn bind(
    app: &MainWindow,
    changed: impl Fn(Option<slint::Weak<AddWindow>>) + 'static,
) -> Rc<RefCell<AddWindowCoordinator>> {
    let coordinator = Rc::new(RefCell::new(AddWindowCoordinator::new()));
    let changed = Rc::new(changed);
    let scheduled = Arc::new(AtomicBool::new(false));
    let parent = app.as_weak();
    app.on_add_window_changed(move || {
        if scheduled.swap(true, Ordering::AcqRel) {
            return;
        }
        let done = scheduled.clone();
        if parent
            .upgrade_in_event_loop(move |app| {
                done.store(false, Ordering::Release);
                app.invoke_sync_add_window();
            })
            .is_err()
        {
            scheduled.store(false, Ordering::Release);
        }
    });
    let host = coordinator.clone();
    let parent = app.as_weak();
    let notify = changed.clone();
    app.on_sync_add_window(move || {
        if let Some(app) = parent.upgrade() {
            let mut host = host.borrow_mut();
            if let Err(error) = host.sync(&app) {
                tracing::warn!(%error,"Cannot open owned add window");
            }
            notify(host.window().map(ComponentHandle::as_weak));
        }
    });
    let host = coordinator.clone();
    let notify = changed;
    app.on_suspend_add_window(move || {
        host.borrow_mut().close();
        notify(None);
    });
    let host = coordinator.clone();
    app.on_focus_add_window(move || {
        if let Some(window) = host.borrow().window() {
            crate::platform::focus(window.window());
        }
    });
    let host = coordinator.clone();
    let parent = app.as_weak();
    app.on_retry_add_window(move || {
        host.borrow_mut().failed = None;
        if let Some(app) = parent.upgrade() {
            app.set_add_window_failed(false);
            app.invoke_add_window_changed();
        }
    });
    coordinator
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::components::MainWindow;
    use slint::ComponentHandle;
    use std::{cell::Cell, rc::Rc};

    struct Headless;
    impl slint::platform::Platform for Headless {
        fn create_window_adapter(
            &self,
        ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
            Ok(slint::platform::software_renderer::MinimalSoftwareWindow::new(Default::default()))
        }
    }
    struct Lease(Rc<Cell<usize>>);
    impl Drop for Lease {
        fn drop(&mut self) {
            self.0.set(self.0.get() - 1);
        }
    }
    fn setup() -> (
        MainWindow,
        AddWindowCoordinator,
        Rc<Cell<usize>>,
        Rc<Cell<usize>>,
    ) {
        let _ = slint::platform::set_platform(Box::new(Headless));
        let app = MainWindow::new().unwrap();
        app.set_ready(true);
        app.show().unwrap();
        let live = Rc::new(Cell::new(0));
        let created = Rc::new(Cell::new(0));
        let counts = (live.clone(), created.clone());
        let coordinator = AddWindowCoordinator::with_attach(Rc::new(move |_, _| {
            counts.0.set(counts.0.get() + 1);
            counts.1.set(counts.1.get() + 1);
            Ok(Box::new(Lease(counts.0.clone())))
        }));
        (app, coordinator, live, created)
    }
    fn preview(app: &MainWindow, key: &str) {
        app.set_show_magnet(false);
        app.set_add_preview_id(key.into());
        app.set_preview_name(format!("Torrent {key}").into());
        app.set_preview_can_confirm(true);
        app.set_preview_open(true);
    }
    #[test]
    fn owns_one_window_through_magnet_preview_then_recreates_for_next_id() {
        let (app, mut coordinator, live, created) = setup();
        coordinator.sync(&app).unwrap();
        assert!(coordinator.window().is_none());
        app.set_show_magnet(true);
        app.set_magnet_link("magnet:test".into());
        coordinator.sync(&app).unwrap();
        assert_eq!(
            coordinator.window().unwrap().get_magnet_link(),
            "magnet:test"
        );
        assert_eq!((live.get(), created.get()), (1, 1));
        preview(&app, "first");
        coordinator.sync(&app).unwrap();
        assert_eq!(
            coordinator.window().unwrap().get_preview_name(),
            "Torrent first"
        );
        assert_eq!((live.get(), created.get()), (1, 1));
        let stale = coordinator.window().unwrap().clone_strong();
        let confirms = Rc::new(Cell::new(0));
        let count = confirms.clone();
        app.on_confirm_preview(move || count.set(count.get() + 1));
        preview(&app, "next");
        coordinator.sync(&app).unwrap();
        assert_eq!((live.get(), created.get()), (1, 2));
        stale.invoke_confirm_preview();
        assert_eq!(
            confirms.get(),
            0,
            "a stale window confirmed the next torrent"
        );
        coordinator.window().unwrap().invoke_confirm_preview();
        assert_eq!(confirms.get(), 1);
        coordinator.close();
        assert_eq!(live.get(), 0);
        stale.invoke_confirm_preview();
        assert_eq!(confirms.get(), 1);
    }
    #[test]
    fn waits_for_native_readiness_without_failing_or_enabling_early_actions() {
        let (app, mut coordinator, live, created) = setup();
        let ready = Rc::new(Cell::new(false));
        let flag = ready.clone();
        coordinator.ready = Rc::new(move |_, _| flag.get());
        preview(&app, "one");
        coordinator.sync(&app).unwrap();
        assert_eq!((live.get(), created.get()), (0, 0));
        assert!(coordinator.window().unwrap().get_busy());
        assert!(!app.get_add_window_failed());
        ready.set(true);
        coordinator.sync(&app).unwrap();
        assert_eq!((live.get(), created.get()), (1, 1));
        assert!(!coordinator.window().unwrap().get_busy());
    }

    #[test]
    fn close_is_guarded_while_busy_and_all_exit_paths_release_native_lease() {
        let (app, mut coordinator, live, _) = setup();
        preview(&app, "one");
        coordinator.sync(&app).unwrap();
        let cancelled = Rc::new(Cell::new(0));
        let count = cancelled.clone();
        app.on_cancel_preview(move || count.set(count.get() + 1));
        app.set_busy(true);
        coordinator.sync(&app).unwrap();
        coordinator
            .window()
            .unwrap()
            .window()
            .dispatch_event(slint::platform::WindowEvent::CloseRequested);
        assert_eq!(cancelled.get(), 0);
        assert_eq!(live.get(), 1);
        app.set_busy(false);
        coordinator.sync(&app).unwrap();
        coordinator
            .window()
            .unwrap()
            .window()
            .dispatch_event(slint::platform::WindowEvent::CloseRequested);
        assert_eq!(cancelled.get(), 1);
        app.set_preview_open(false);
        coordinator.sync(&app).unwrap();
        assert_eq!(live.get(), 0);
        preview(&app, "two");
        coordinator.sync(&app).unwrap();
        app.set_stopping(true);
        coordinator.sync(&app).unwrap();
        assert_eq!(live.get(), 0);
        assert!(coordinator.window().is_none());
    }
    #[test]
    fn native_failure_preserves_cancel_route_without_repeated_creation() {
        let (app, _, _, _) = setup();
        let attempts = Rc::new(Cell::new(0));
        let count = attempts.clone();
        let mut coordinator = AddWindowCoordinator::with_attach(Rc::new(move |_, _| {
            count.set(count.get() + 1);
            Err(anyhow::anyhow!("native ownership unavailable").into())
        }));
        preview(&app, "one");
        assert!(coordinator.sync(&app).is_err());
        assert!(app.get_add_window_failed());
        assert!(app.get_preview_open());
        assert!(coordinator.window().is_none());
        coordinator.sync(&app).unwrap();
        assert_eq!(attempts.get(), 1);
        app.set_preview_open(false);
        coordinator.sync(&app).unwrap();
        preview(&app, "two");
        assert!(coordinator.sync(&app).is_err());
        assert_eq!(attempts.get(), 2);
    }
}
