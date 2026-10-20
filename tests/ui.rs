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
    assert!((look.min - 20.0).abs() < 0.2, "min moved to {}", look.min);
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
fn open_shows_the_file_browser_without_waiting_for_more_input() {
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
    for _ in 0..3 {
        assert!(h.ctx.has_requested_repaint(), "nothing asks for the frame that draws the browser");
        h.frame(Vec::new());
    }
    assert!(h.texts().iter().any(|(t, _)| t.starts_with("FOLDER")));
}
