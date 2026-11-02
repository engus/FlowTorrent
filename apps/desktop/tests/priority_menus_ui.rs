use flowtorrent::ui::components::{DetailFileRow, MainWindow, TorrentRow, Translations};
use slint::{
    ComponentHandle, ModelRc,
    platform::{
        Platform, PlatformError, WindowAdapter, WindowEvent,
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    },
};
use std::{cell::RefCell, rc::Rc};
struct Rendered(Rc<MinimalSoftwareWindow>);
impl Platform for Rendered {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
}
#[test]
fn file_and_torrent_priority_menus_choose_without_changing_on_open_and_respect_busy() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Rendered(window.clone()))).unwrap();
    let app = MainWindow::new().unwrap();
    app.global::<Translations>().on_lookup(|_, _, en| en);
    app.window()
        .set_size(slint::LogicalSize::new(1000.0, 720.0));
    app.set_ready(true);
    app.set_selected_count(1);
    app.set_file_selection_enabled(true);
    app.set_selected_row(TorrentRow {
        id: "torrent".into(),
        name: "Bundle".into(),
        priority: 1,
        can_act: true,
        ..Default::default()
    });
    app.on_context_requested(|_| true);
    let file = DetailFileRow {
        file_id: 7,
        name: "actual.bin".into(),
        path: "folder/actual.bin".into(),
        priority: 1,
        selected: false,
        ..Default::default()
    };
    app.set_detail_files(ModelRc::from([file.clone()]));
    let files = Rc::new(RefCell::new(Vec::new()));
    let calls = files.clone();
    app.on_file_priority(move |id, file, value| {
        calls.borrow_mut().push((id.to_string(), file, value))
    });
    let torrents = Rc::new(RefCell::new(Vec::new()));
    let calls = torrents.clone();
    app.on_torrent_priority(move |id, value| calls.borrow_mut().push((id.to_string(), value)));
    app.show().unwrap();
    let draw = || {
        slint::platform::update_timers_and_animations();
        window.draw_if_needed(|r| {
            r.render(&mut vec![slint::Rgb8Pixel::default(); 1000 * 720], 1000);
        });
    };
    let key = |key: slint::SharedString| {
        app.window()
            .dispatch_event(WindowEvent::KeyPressed { text: key.clone() });
        app.window()
            .dispatch_event(WindowEvent::KeyReleased { text: key });
        draw();
    };
    draw();
    app.invoke_show_file_context(
        "torrent".into(),
        file.clone(),
        slint::LogicalPosition::new(120.0, 160.0),
    );
    draw();
    assert!(files.borrow().is_empty(), "opening changed a priority");
    key(slint::platform::Key::UpArrow.into());
    key(slint::platform::Key::Return.into());
    assert_eq!(*files.borrow(), [("torrent".into(), 7, 2)]);
    app.invoke_show_context("torrent".into(), slint::LogicalPosition::new(160.0, 100.0));
    draw();
    assert!(torrents.borrow().is_empty());
    // The menu's priority picker is reached after the existing torrent actions.
    key(slint::platform::Key::UpArrow.into());
    key(slint::platform::Key::Return.into());
    assert_eq!(*torrents.borrow(), [("torrent".into(), 2)]);
    app.set_busy(true);
    app.invoke_show_file_context(
        "torrent".into(),
        file,
        slint::LogicalPosition::new(120.0, 160.0),
    );
    draw();
    key(slint::platform::Key::UpArrow.into());
    key(slint::platform::Key::Return.into());
    assert_eq!(files.borrow().len(), 1);
}

#[test]
fn torrent_priority_icon_aligns_with_the_state_icon() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Rendered(window.clone()))).unwrap();
    let app = MainWindow::new().unwrap();
    app.global::<Translations>().on_lookup(|_, _, en| en);
    app.window()
        .set_size(slint::LogicalSize::new(1280.0, 720.0));
    app.invoke_apply_theme(1);
    app.set_ready(true);
    app.set_macos(true);
    app.set_sidebar_collapsed(true);
    app.set_rows(ModelRc::from([TorrentRow {
        id: "torrent".into(),
        name: "Bundle".into(),
        priority: 1,
        selected: true,
        ..Default::default()
    }]));
    app.show().unwrap();
    slint::platform::update_timers_and_animations();
    let mut pixels = vec![slint::Rgb8Pixel::default(); 1280 * 720];
    assert!(window.draw_if_needed(|r| {
        r.render(&mut pixels, 1280);
    }));
    if let Ok(path) = std::env::var("FLOWTORRENT_CAPTURE_PRIORITY") {
        let mut ppm = b"P6\n1280 720\n255\n".to_vec();
        for p in &pixels {
            ppm.extend([p.r, p.g, p.b]);
        }
        std::fs::write(path, ppm).unwrap();
    }
    // Compare the actual SVG marks in the rendered row; a focus/hit rectangle
    // can include the row center while the glyph itself is top-aligned.
    let state_y: Vec<_> = (100..260)
        .filter(|y| {
            (82..104).any(|x| {
                let p = pixels[y * 1280 + x];
                u16::from(p.g.max(p.b)) > u16::from(p.r) + 40
            })
        })
        .collect();
    assert!(!state_y.is_empty(), "state mark is missing");
    let state_center = (state_y[0] + state_y.last().unwrap() + 1) as f32 / 2.0;
    let priority_y: Vec<_> = (state_center as usize - 40..state_center as usize + 40)
        .filter(|y| {
            (640..704).any(|x| {
                let p = pixels[y * 1280 + x];
                p.r < 100 && p.g < 100 && p.b < 100
            })
        })
        .collect();
    assert!(!priority_y.is_empty(), "priority mark is missing");
    let priority_center = (priority_y[0] + priority_y.last().unwrap() + 1) as f32 / 2.0;
    assert!(
        (priority_center - state_center).abs() <= 1.0,
        "priority glyph center {priority_center} differs from row state {state_center}"
    );
}
