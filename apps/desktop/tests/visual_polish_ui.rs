use flowtorrent::ui::components::{
    AddWindow, MainWindow, PreviewFileRow, TorrentRow, Translations,
};
use slint::{
    ComponentHandle, ModelRc, Rgb8Pixel,
    platform::{
        Platform, PlatformError, WindowAdapter, WindowEvent,
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    },
};
use std::{collections::HashSet, rc::Rc};

struct Rendered(Rc<MinimalSoftwareWindow>);
impl Platform for Rendered {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
}

#[test]
fn add_confirmation_primary_button_shows_keyboard_focus_in_both_themes() {
    const WIDTH: usize = 640;
    const HEIGHT: usize = 560;
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Rendered(window.clone()))).unwrap();
    let app = AddWindow::new().unwrap();
    app.global::<Translations>().on_lookup(|_, _, en| en);
    app.window()
        .set_size(slint::LogicalSize::new(WIDTH as f32, HEIGHT as f32));
    app.set_preview_open(true);
    app.set_preview_name("Bundle".into());
    app.set_preview_folder("/Downloads".into());
    app.set_preview_final_path("/Downloads/Bundle".into());
    app.set_preview_size("1 MB".into());
    app.set_preview_can_confirm(true);
    app.set_preview_files(ModelRc::from([PreviewFileRow {
        id: 7,
        name: "actual.bin".into(),
        size: "1 MB".into(),
        checked: true,
    }]));
    app.show().unwrap();
    let draw = |pixels: &mut [Rgb8Pixel]| {
        slint::platform::update_timers_and_animations();
        window.draw_if_needed(|renderer| {
            renderer.render(pixels, WIDTH);
        });
    };
    for dark in [false, true] {
        app.set_dark(dark);
        let mut pixels = vec![Rgb8Pixel::default(); WIDTH * HEIGHT];
        draw(&mut pixels);
        let primary: Vec<_> = (HEIGHT / 2..HEIGHT)
            .flat_map(|y| (WIDTH / 2..WIDTH).map(move |x| (x, y)))
            .filter(|(x, y)| {
                let p = pixels[y * WIDTH + x];
                u16::from(p.g) > u16::from(p.r) + 30 && u16::from(p.b) > u16::from(p.r) + 30
            })
            .collect();
        assert!(!primary.is_empty(), "confirmation button is missing");
        let left = primary.iter().map(|(x, _)| *x).min().unwrap() - 4;
        let right = primary.iter().map(|(x, _)| *x).max().unwrap() + 5;
        let top = primary.iter().map(|(_, y)| *y).min().unwrap() - 4;
        let bottom = primary.iter().map(|(_, y)| *y).max().unwrap() + 5;
        let mut appearances = HashSet::new();
        for _ in 0..10 {
            appearances.insert(
                (top..bottom)
                    .flat_map(|y| &pixels[y * WIDTH + left..y * WIDTH + right])
                    .flat_map(|p| [p.r, p.g, p.b])
                    .collect::<Vec<_>>(),
            );
            let text: slint::SharedString = slint::platform::Key::Tab.into();
            app.window()
                .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
            app.window()
                .dispatch_event(WindowEvent::KeyReleased { text });
            draw(&mut pixels);
        }
        assert!(
            appearances.len() > 1,
            "keyboard traversal never gives the primary button a visible focus state (dark={dark})"
        );
    }
}

#[test]
fn compact_error_row_keeps_its_error_text_inside_the_row() {
    const WIDTH: usize = 800;
    const HEIGHT: usize = 640;
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Rendered(window.clone()))).unwrap();
    let app = MainWindow::new().unwrap();
    app.global::<Translations>().on_lookup(|_, _, en| en);
    app.window()
        .set_size(slint::LogicalSize::new(WIDTH as f32, HEIGHT as f32));
    app.invoke_apply_theme(1);
    app.set_macos(true);
    app.set_ready(true);
    app.set_sidebar_width(280.0);
    app.set_rows(ModelRc::from([TorrentRow {
        id: "error".into(),
        name: "An archive with a long filename".into(),
        status: "Error".into(),
        error: "Cannot write to this folder.".into(),
        selected: true,
        stage: 4,
        ..Default::default()
    }]));
    app.show().unwrap();
    slint::platform::update_timers_and_animations();
    let mut pixels = vec![Rgb8Pixel::default(); WIDTH * HEIGHT];
    window.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, WIDTH);
    });
    let selected: Vec<_> = (0..HEIGHT)
        .filter(|y| {
            let p = pixels[y * WIDTH + WIDTH - 2];
            u16::from(p.g.min(p.b)) > u16::from(p.r) + 4
        })
        .collect();
    let top = *selected.first().expect("selected row is missing");
    let bottom = *selected.last().unwrap() + 1;
    let error: Vec<_> = (top..(bottom + 20).min(HEIGHT))
        .filter(|y| {
            (320..WIDTH - 40).any(|x| {
                let p = pixels[y * WIDTH + x];
                u16::from(p.r) > u16::from(p.g.max(p.b)) + 30
            })
        })
        .collect();
    assert!(!error.is_empty(), "actionable error text is missing");
    assert!(
        *error.last().unwrap() < bottom - 3,
        "error text extends to row edge: last ink {}, row ends {bottom}",
        error.last().unwrap()
    );
}
