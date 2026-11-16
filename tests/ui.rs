use egui::pos2;
use thermview::headless::Harness;
use thermview::host::Loaded;

fn synthetic() -> Loaded {
    let (w, h) = (64u32, 48u32);
    let mut d = vec![0xFF, 0xD8, 0xFF, 0xD9];
    let mut hdri = vec![0u8; 44];
    hdri[0..4].copy_from_slice(b"HDRI");
    hdri[4..6].copy_from_slice(&0x1028u16.to_le_bytes());
    hdri[12..16].copy_from_slice(&w.to_le_bytes());
    hdri[16..20].copy_from_slice(&h.to_le_bytes());
    d.extend(hdri);
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - 40.0;
            let dy = y as f32 - 20.0;
            let c = 20.0 + 30.0 * (-(dx * dx + dy * dy) / 120.0).exp();
            d.extend(((c * 100.0) as u16).to_le_bytes());
        }
    }
    Loaded { name: "synthetic.jpg".into(), bytes: d, modified: None, path: None }
}

fn harness() -> Harness {
    let mut h = Harness::new(1400.0, 900.0);
    h.settle();
    let ctx = h.ctx.clone();
    h.app.load(&ctx, Ok(synthetic()));
    h.settle();
    let look = h.find("LOOK").expect("image controls activity").center();
    h.click(look);
    h.settle();
    h
}

fn save(h: &Harness, name: &str) {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    std::fs::write(dir.join(name), h.png()).unwrap();
}

#[test]
fn clicking_the_image_drops_a_point_that_reads_the_pixel_under_it() {
    let mut h = harness();
    let hottest = h.find("MAX: 50.0°C").expect("max marker drawn");
    let near = pos2(hottest.center().x, hottest.bottom() + 40.0);
    h.click(near);
    let doc = h.app.doc.as_ref().unwrap();
    assert_eq!(doc.points.len(), 1);
    let pt = &doc.points[0];
    let label = format!("1: {}", doc.unit.reading(doc.reading(pt)));
    assert!(h.find(&label).is_some(), "{label} not drawn");
    save(&h, "point.png");
}

#[test]
fn dragging_the_top_handle_lowers_the_window() {
    let mut h = harness();
    let before = h.app.doc.as_ref().unwrap().look.max;
    let (t45, t40) = (h.find("45").expect("tick 45"), h.find("40").expect("tick 40"));
    let bar_x = t45.left() - 7.0 - 9.0;
    let top = t45.center().y - (t40.center().y - t45.center().y);
    h.drag(pos2(bar_x, top), pos2(bar_x, top + 150.0));
    let look = h.app.doc.as_ref().unwrap().look;
    assert!(look.max < before - 5.0, "max stayed at {}", look.max);
    assert!((look.min - 5.0).abs() < 0.2, "min moved to {}", look.min);
    save(&h, "drag.png");
}

#[test]
fn picking_a_palette_recolours_the_image() {
    let mut h = harness();
    let at = h.find("JET").expect("jet listed").center();
    h.click(at);
    assert_eq!(h.app.doc.as_ref().unwrap().look.palette, thermview::palette::Palette::Jet);
    save(&h, "jet.png");
}

#[test]
fn panels_hold_their_width() {
    let mut h = harness();
    let edge = |h: &Harness| h.find("the window the palette spans").map(|r| r.right());
    let first = edge(&h);
    for _ in 0..20 {
        h.frame(Vec::new());
    }
    assert_eq!(edge(&h), first);
}

#[test]
fn a_flir_jpeg_shows_its_calibration_and_camera() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test-samples/flir_rjpeg_example.jpg");
    if !path.exists() {
        eprintln!("skipping, {} is not checked out", path.display());
        return;
    }
    let mut h = Harness::new(1500.0, 1000.0);
    h.settle();
    let ctx = h.ctx.clone();
    h.app.load(&ctx, thermview::host::read(&path));
    h.settle();
    assert!(h.find("CALIBRATION").is_some());
    assert!(h.find("2017-09-08 16:04").is_some());
    let max = h.find("MAX: 62.3°C").expect("max marker");
    h.click(pos2(max.center().x - 60.0, max.center().y - 120.0));
    save(&h, "flir.png");
}

#[test]
fn the_empty_window_offers_to_open_a_file() {
    let mut h = Harness::new(1200.0, 800.0);
    h.settle();
    let infiray = h.find("INFIRAY").expect("formats listed");
    let dji = h.find("DJI").expect("formats listed");
    assert_eq!(infiray.left(), dji.left());
    save(&h, "welcome.png");
}

#[test]
fn open_requests_the_native_file_dialog_without_waiting_for_more_input() {
    let mut h = Harness::new(1200.0, 800.0);
    h.settle();
    let at = h.find("OPEN").expect("open button").center();
    h.frame(vec![egui::Event::PointerMoved(at)]);
    h.settle();
    h.frame(vec![egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    }]);
    h.frame(vec![egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    }]);
    #[cfg(not(target_arch = "wasm32"))]
    assert!(h.app.file_dialog.busy(), "native file picker was not requested");
    assert!(h.app.doc.is_none());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn activity_bar_switches_and_collapses_the_sidebar() {
    let mut h = harness();
    assert!(h.find("PALETTE").is_some());
    let files = h.find("FILES").unwrap().center();
    h.click(files);
    assert!(h.find("EXPLORER").is_some());
    assert!(h.find("PALETTE").is_none());
    h.click(files);
    assert!(h.find("EXPLORER").is_none());
    assert!(h.find("PALETTE").is_none());
    h.click(h.find("LOOK").unwrap().center());
    assert!(h.find("PALETTE").is_some());
    assert!(h.app.doc.is_some());
    save(&h, "activity-controls.png");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn explorer_opens_nested_thermal_files_and_reports_unsupported_files() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("explorer-ui");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("nested")).unwrap();
    std::fs::write(dir.join("nested/scene.jpg"), synthetic().bytes).unwrap();
    std::fs::write(dir.join("notes.txt"), "not a thermal image").unwrap();
    std::fs::write(dir.join(".hidden"), "hidden file").unwrap();
    let mut h = harness();
    h.app.explorer.set_root(dir.clone());
    h.click(h.find("FILES").unwrap().center());
    assert!(h.find(".hidden").is_some(), "hidden files are listed");
    assert!(h.find("scene.jpg").is_none(), "folder starts collapsed");
    h.click(h.find("nested").unwrap().center());
    h.click(h.find("scene.jpg").unwrap().center());
    assert_eq!(h.app.doc.as_ref().unwrap().name, "scene.jpg");
    assert_eq!(h.app.explorer.root, dir.canonicalize().unwrap());
    h.click(h.find("notes.txt").unwrap().center());
    assert_eq!(h.app.doc.as_ref().unwrap().name, "scene.jpg", "failed open keeps the current view");
    assert!(h.texts().iter().any(|(t, _)| t.contains("Cannot open notes.txt")));
    save(&h, "explorer.png");
    std::fs::write(dir.join("new-file.txt"), "new file").unwrap();
    assert!(h.find("new-file.txt").is_none());
    h.click(h.find("REFRESH").unwrap().center());
    assert!(h.find("new-file.txt").is_some());
    h.click(h.find("nested").unwrap().center());
    assert!(h.find("scene.jpg").is_none(), "folder collapses");
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_file_and_folder_choices_load_without_a_custom_modal() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("native-dialog-ui");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("thermal ü with spaces.jpg");
    std::fs::write(&path, synthetic().bytes).unwrap();
    let mut h = Harness::new(1400.0, 900.0);
    h.settle();
    h.app.explorer.set_root(dir.clone());
    h.app.file_dialog = thermview::dialog::NativePicker::with_backend(|_, dir| {
        Box::pin(std::future::ready(Some(dir.join("thermal ü with spaces.jpg"))))
    });
    h.click(h.find("OPEN").unwrap().center());
    assert_eq!(h.app.doc.as_ref().unwrap().name, "thermal ü with spaces.jpg");
    let image = h.app.doc.as_ref().unwrap().image.celsius.clone();
    h.app.file_dialog =
        thermview::dialog::NativePicker::with_backend(|_, _| Box::pin(std::future::ready(None)));
    h.click(h.find("OPEN").unwrap().center());
    assert_eq!(
        h.app.doc.as_ref().unwrap().image.celsius,
        image,
        "cancelling keeps the current image"
    );
    h.app.file_dialog = thermview::dialog::NativePicker::with_backend(|_, dir| {
        Box::pin(std::future::ready(Some(dir.join("subfolder"))))
    });
    std::fs::create_dir_all(dir.join("subfolder")).unwrap();
    h.click(h.find("OPEN FOLDER").unwrap().center());
    assert_eq!(h.app.explorer.root, dir.join("subfolder").canonicalize().unwrap());
    assert_eq!(h.app.doc.as_ref().unwrap().image.celsius, image);
    let _ = std::fs::remove_dir_all(dir);
}

fn wheel(at: egui::Pos2, delta: egui::Vec2, modifiers: egui::Modifiers) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(at),
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta,
            modifiers,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
        },
    ]
}

#[test]
fn ctrl_wheel_zooms_the_image_and_plain_wheel_pans_without_changing_export_scale() {
    let mut h = harness();
    let viewport = h.thermal_rect().unwrap();
    let at = viewport.center();
    let export_scale = h.app.doc.as_ref().unwrap().scale;
    let app_zoom = h.ctx.zoom_factor();
    h.frame(wheel(at, egui::vec2(0.0, 400.0), egui::Modifiers::CTRL));
    h.settle();
    let view = h.app.doc.as_ref().unwrap().view;
    assert!(view.zoom > 2.0);
    assert_eq!(h.ctx.zoom_factor(), app_zoom, "image zoom must not zoom the interface");
    assert_eq!(h.app.doc.as_ref().unwrap().scale, export_scale);
    h.frame(wheel(at, egui::vec2(0.0, -100.0), egui::Modifiers::NONE));
    let panned = h.app.doc.as_ref().unwrap().view;
    assert_eq!(panned.zoom, view.zoom);
    assert!(panned.center.y > view.center.y);
    h.frame(wheel(at, egui::vec2(0.0, -100.0), egui::Modifiers::SHIFT));
    assert!(h.app.doc.as_ref().unwrap().view.center.x > panned.center.x);
    h.click(h.find("RESET VIEW").unwrap().center());
    assert_eq!(h.app.doc.as_ref().unwrap().view, thermview::view::ImageView::default());
    assert!(h.find("1X").is_none() && h.find("6X").is_none());
    assert!(h.find("IRON").unwrap().top() < h.find("INFERNO").unwrap().top());
    save(&h, "v021-default-view.png");
}

#[test]
fn clicking_a_zoomed_pixel_reads_that_pixel_and_hot_labels_are_red() {
    let mut h = harness();
    let at = h.thermal_rect().unwrap().center();
    h.frame(wheel(at, egui::vec2(0.0, 400.0), egui::Modifiers::CTRL));
    h.settle();
    let rect = h.thermal_rect().unwrap();
    // The image centre is the source centre despite zoom and clipping.
    h.click(at);
    let doc = h.app.doc.as_ref().unwrap();
    let point = doc.points.last().unwrap();
    let expected_x =
        ((at.x - rect.left()) / rect.width() * doc.image.width as f32).floor() as usize;
    let expected_y =
        ((at.y - rect.top()) / rect.height() * doc.image.height as f32).floor() as usize;
    assert_eq!((point.x, point.y), (expected_x, expected_y));
    assert_eq!(doc.reading(point), doc.image.at(expected_x, expected_y).unwrap());
    h.click(h.find("RESET VIEW").unwrap().center());
    assert!(h.text_colors("50.0").contains(&thermview::overlay::HOT));
    assert!(h.text_colors("20.0").contains(&thermview::overlay::COLD));
    save(&h, "v021-hot-cold.png");
}
