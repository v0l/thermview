#![cfg(not(target_arch = "wasm32"))]

use thermview::doc::Doc;
use thermview::headless::Harness;
use thermview::host::Loaded;
use thermview::image::Format;
use thermview::video::Video;

// Two independently compressed radiometric frames, including both FFF directory
// byte orders. The encoded data never needs an external video tool at runtime.
fn frame(counts: &[u16], little: bool) -> Vec<u8> {
    let encoded = charls::CharLS::default()
        .encode(
            charls::FrameInfo { width: 8, height: 6, bits_per_sample: 16, component_count: 1 },
            0,
            &counts.iter().flat_map(|c| c.to_ne_bytes()).collect::<Vec<_>>(),
        )
        .unwrap();
    let mut raw = vec![0; 0x20];
    raw[0..2].copy_from_slice(&2u16.to_le_bytes());
    raw[2..4].copy_from_slice(&8u16.to_le_bytes());
    raw[4..6].copy_from_slice(&6u16.to_le_bytes());
    raw.extend(encoded);
    let mut camera = vec![0; 0x468];
    camera[0..2].copy_from_slice(&2u16.to_le_bytes());
    for (offset, value) in [
        (0x20, 0.95f32),
        (0x24, 1.0),
        (0x28, 293.15),
        (0x2c, 293.15),
        (0x3c, 0.5),
        (0x58, 21106.77),
        (0x5c, 1501.0),
        (0x60, 1.0),
        (0x30c, 0.012545258),
    ] {
        camera[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    camera[0x308..0x30c].copy_from_slice(&(-7340i32).to_le_bytes());
    camera[0x464..0x466].copy_from_slice(&25u16.to_le_bytes());
    let u32_bytes = |v: u32| if little { v.to_le_bytes() } else { v.to_be_bytes() };
    let u16_bytes = |v: u16| if little { v.to_le_bytes() } else { v.to_be_bytes() };
    let mut bytes = vec![0; 0x80];
    bytes[..6].copy_from_slice(b"FFF\0RT");
    bytes[0x14..0x18].copy_from_slice(&u32_bytes(100));
    bytes[0x18..0x1c].copy_from_slice(&u32_bytes(0x40));
    bytes[0x1c..0x20].copy_from_slice(&u32_bytes(2));
    for (entry, kind, offset, length) in
        [(0x40, 1, 0x80, raw.len()), (0x60, 32, 0x80 + raw.len(), camera.len())]
    {
        bytes[entry..entry + 2].copy_from_slice(&u16_bytes(kind));
        bytes[entry + 12..entry + 16].copy_from_slice(&u32_bytes(offset as u32));
        bytes[entry + 16..entry + 20].copy_from_slice(&u32_bytes(length as u32));
    }
    bytes.extend(raw);
    bytes.extend(camera);
    bytes
}

fn sequence() -> Vec<u8> {
    let a: Vec<u16> = (0..48).map(|i| 14000 + i * 10).collect();
    let b: Vec<u16> = (0..48).map(|i| 16000 + (47 - i) * 20).collect();
    [frame(&a, true), frame(&b, false)].concat()
}

#[test]
fn compressed_frames_seek_in_both_directions_with_fresh_extremes() {
    let video = Video::open(sequence()).unwrap();
    assert_eq!(video.len(), 2);
    assert_eq!(video.fps, 25.0);
    assert!(!video.estimated_fps);
    assert_eq!(video.frame(1).unwrap().radiometry.unwrap().counts[0], 16940);
    let mut doc = Doc::from_video(video, "clip.CSQ", None).unwrap();
    assert_eq!(doc.image.format, Format::FlirCsq);
    let first = doc.image.celsius.clone();
    assert_eq!(doc.image.stats.min.x, 0);
    doc.add_point(3, 2);
    doc.seek(1).unwrap();
    assert_eq!(doc.image.stats.max.x, 0);
    assert_eq!(doc.image.stats.min.x, 7);
    assert_eq!(doc.image.stats.min.y, 5);
    assert_eq!(doc.look.max, doc.image.stats.max.celsius);
    assert_ne!(doc.image.celsius, first);
    assert_eq!(doc.reading(&doc.points[0]), doc.image.at(3, 2).unwrap());
    assert_eq!(thermview::export::file_name(&doc), "clip-frame-000002-thermview.png");
    doc.seek(0).unwrap();
    assert_eq!(doc.image.celsius, first);
    assert_eq!(doc.revision, 2);
    assert!(doc.seek(2).is_err());
    assert_eq!(doc.video.as_ref().unwrap().current, 0);
}

#[test]
fn locked_palette_and_calibration_survive_seeking() {
    let mut doc = Doc::from_video(Video::open(sequence()).unwrap(), "clip.csq", None).unwrap();
    doc.video.as_mut().unwrap().auto_range = false;
    doc.look.min = 0.0;
    doc.look.max = 100.0;
    let mut params = doc.params();
    params.emissivity = 0.8;
    doc.set_params(params);
    doc.seek(1).unwrap();
    assert_eq!(doc.params(), params);
    assert_eq!((doc.look.min, doc.look.max), (0.0, 100.0));
}

#[test]
fn truncated_and_empty_sequences_are_rejected() {
    assert!(Video::open(Vec::new()).is_err());
    let mut bytes = sequence();
    bytes.truncate(bytes.len() - 30);
    assert!(Video::open(bytes).is_err());
}

#[test]
fn video_controls_scrub_and_disappear_when_an_image_is_opened() {
    let mut h = Harness::new(1500.0, 950.0);
    h.settle();
    h.app.load(
        &h.ctx.clone(),
        Ok(Loaded { name: "clip.csq".into(), bytes: sequence(), modified: None, path: None }),
    );
    h.settle();
    assert!(h.find("PLAY").is_some());
    let label = h.find("FRAME 1 / 2").unwrap();
    // The ruler sits immediately below the transport row and spans the window.
    h.click(egui::pos2(1450.0, label.bottom() + 30.0));
    assert_eq!(h.app.doc.as_ref().unwrap().video.as_ref().unwrap().current, 1);
    assert!(h.find("FRAME 2 / 2").is_some());
    let max = h
        .app
        .doc
        .as_ref()
        .unwrap()
        .unit
        .reading(h.app.doc.as_ref().unwrap().image.stats.max.celsius);
    assert!(h.find(&format!("MAX: {max}")).is_some());
    std::fs::write(std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("csq.png"), h.png())
        .unwrap();
    let image = thermview::ThermalImage::new(Format::Hikmicro, 8, 6, vec![20.0; 48]);
    h.app.doc = Some(Doc::new(image, "still.jpg", None));
    h.settle();
    assert!(h.find("PLAY").is_none());
    assert!(h.find("FRAME 2 / 2").is_none());
    assert!(!h.texts().iter().any(|(text, _)| text.starts_with("VIEW ")));
}

#[test]
#[ignore = "set THERMVIEW_TEST_CSQ to a local camera recording"]
fn real_camera_recording_opens_and_seeks() {
    let path = std::env::var("THERMVIEW_TEST_CSQ").expect("set THERMVIEW_TEST_CSQ");
    let video = Video::open(std::fs::read(&path).unwrap()).unwrap();
    let count = video.len();
    println!("Indexed {count} frames at {} fps", video.fps);
    let mut doc = Doc::from_video(video, &path, None).unwrap();
    for index in [0, count / 2, count - 1, 0] {
        doc.seek(index).unwrap();
        assert!(doc.image.stats.min.celsius.is_finite());
        assert!(doc.image.stats.max.celsius.is_finite());
        assert_eq!(doc.image.celsius.len(), doc.image.width * doc.image.height);
        println!(
            "Frame {}: {} x {}, min {:.2} C, max {:.2} C",
            index + 1,
            doc.image.width,
            doc.image.height,
            doc.image.stats.min.celsius,
            doc.image.stats.max.celsius
        );
    }
}

#[test]
#[ignore = "set THERMVIEW_TEST_CSQ and THERMVIEW_TEST_REFERENCE to the recording and independent raw decode"]
fn real_camera_padding_frame_matches_independent_decoder() {
    let path = std::env::var("THERMVIEW_TEST_CSQ").unwrap();
    let reference = std::env::var("THERMVIEW_TEST_REFERENCE").unwrap();
    let expected = std::fs::read(reference).unwrap();
    let expected: Vec<u16> =
        expected.chunks_exact(2).map(|p| u16::from_le_bytes([p[0], p[1]])).collect();
    let mut doc =
        Doc::from_video(Video::open(std::fs::read(path).unwrap()).unwrap(), "recording.csq", None)
            .unwrap();
    doc.seek(11146).unwrap();
    doc.seek(11147).unwrap();
    assert_eq!(doc.image.radiometry.as_ref().unwrap().counts, expected);
    assert_eq!(doc.video.as_ref().unwrap().current, 11147);
    println!("Frame 11148: every sensor count matches the independent decoder");
    doc.seek(11148).unwrap();
    assert_eq!(doc.video.as_ref().unwrap().current, 11148);
}

#[test]
#[ignore = "set THERMVIEW_TEST_CSQ; decodes every frame of a local recording"]
fn every_frame_of_real_camera_recording_decodes() {
    let path = std::env::var("THERMVIEW_TEST_CSQ").unwrap();
    let video = Video::open(std::fs::read(path).unwrap()).unwrap();
    for index in 0..video.len() {
        video.frame(index).unwrap();
    }
    println!("All {} frames decoded successfully", video.len());
}

fn zoom_harness() -> Harness {
    let bytes: Vec<u8> = (0..101)
        .flat_map(|index| {
            let counts: Vec<u16> = (0..48).map(|i| 14000 + index * 20 + i * 10).collect();
            frame(&counts, true)
        })
        .collect();
    let mut h = Harness::new(1500.0, 950.0);
    h.settle();
    h.app.load(
        &h.ctx.clone(),
        Ok(Loaded { name: "zoom.csq".into(), bytes, modified: None, path: None }),
    );
    h.settle();
    h
}

#[test]
fn navigator_handles_zoom_and_middle_pans_without_seeking() {
    let mut h = zoom_harness();
    let nav_y = h.find("VIEW 1–101").unwrap().top() - 18.0;
    h.drag(egui::pos2(1475.0, nav_y), egui::pos2(760.0, nav_y));
    let video = h.app.doc.as_ref().unwrap().video.as_ref().unwrap();
    assert_eq!(video.current, 0, "resizing the view must not seek");
    assert!((35..65).contains(&video.view.last()), "right handle must zoom: {:?}", video.view);
    assert_eq!(video.view.first(), 0);
    h.drag(egui::pos2(25.0, nav_y), egui::pos2(200.0, nav_y));
    let video = h.app.doc.as_ref().unwrap().video.as_ref().unwrap();
    let before = video.view;
    assert!(before.first() > 0, "left handle must move the range start");
    let center = 20.0 + 1460.0 * (before.first() + before.last()) as f32 / 200.0;
    h.drag(egui::pos2(center, nav_y), egui::pos2(center + 180.0, nav_y));
    let video = h.app.doc.as_ref().unwrap().video.as_ref().unwrap();
    assert_eq!(video.current, 0, "panning must leave the displayed frame alone");
    assert_eq!(video.view.span(), before.span(), "panning must preserve zoom");
    assert!(video.view.first() > before.first());
    let range = video.view;
    // Tick labels are in the detailed ruler above the navigator.
    let ruler_y = h.find(&(range.first() + 1).to_string()).unwrap().center().y + 24.0;
    h.click(egui::pos2(750.0, ruler_y));
    let doc = h.app.doc.as_ref().unwrap();
    let video = doc.video.as_ref().unwrap();
    let midpoint = (range.first() + range.last()) / 2;
    assert!(video.current.abs_diff(midpoint) <= 1, "scrubbing must map to the visible range");
    assert_eq!(video.view, range);
    let extrema = format!(
        "MIN {}    MAX {}",
        doc.unit.reading(doc.image.stats.min.celsius),
        doc.unit.reading(doc.image.stats.max.celsius)
    );
    assert!(
        h.find(&extrema).is_some(),
        "per-frame temperatures must update after zoomed scrubbing"
    );
    std::fs::write(std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("csq-zoom.png"), h.png())
        .unwrap();
    h.click(h.find("FIT").unwrap().center());
    let video = h.app.doc.as_ref().unwrap().video.as_ref().unwrap();
    assert_eq!((video.view.first(), video.view.last()), (0, 100));
    assert!(video.current.abs_diff(midpoint) <= 1, "Fit must not seek");
}

#[test]
fn zoom_buttons_and_wheel_do_not_seek_and_transport_keeps_zoom() {
    let mut h = zoom_harness();
    h.click(h.find("+").unwrap().center());
    let video = h.app.doc.as_ref().unwrap().video.as_ref().unwrap();
    assert_eq!(video.current, 0);
    assert_eq!(video.view.span(), 50);
    let label = format!("VIEW {}–{}", video.view.first() + 1, video.view.last() + 1);
    let nav_y = h.find(&label).unwrap().top() - 18.0;
    h.frame(vec![
        egui::Event::PointerMoved(egui::pos2(400.0, nav_y)),
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            delta: egui::vec2(0.0, 120.0),
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    h.settle();
    let video = h.app.doc.as_ref().unwrap().video.as_ref().unwrap();
    assert!(video.view.span() < 50, "wheel must zoom the navigator");
    assert_eq!(video.current, 0);
    let span = video.view.span();
    h.click(h.find(">|").unwrap().center());
    let video = h.app.doc.as_ref().unwrap().video.as_ref().unwrap();
    assert_eq!(video.current, 100);
    assert_eq!(video.view.last(), 100);
    assert_eq!(video.view.span(), span, "transport follows the frame without resetting zoom");
    h.click(h.find("FIT").unwrap().center());
    assert_eq!(h.app.doc.as_ref().unwrap().video.as_ref().unwrap().view.span(), 100);
}

#[test]
fn maximum_zoom_stays_precise_and_new_video_resets_the_view() {
    let mut h = zoom_harness();
    for _ in 0..8 {
        h.click(h.find("+").unwrap().center());
    }
    let view = h.app.doc.as_ref().unwrap().video.as_ref().unwrap().view;
    assert_eq!(view.span(), 1);
    let y = h.find("1").unwrap().center().y + 24.0;
    h.click(egui::pos2(1475.0, y));
    assert_eq!(
        h.app.doc.as_ref().unwrap().video.as_ref().unwrap().current,
        1,
        "full-width scrub at maximum zoom should step exactly one frame"
    );
    h.app.load(
        &h.ctx.clone(),
        Ok(Loaded { name: "new.csq".into(), bytes: sequence(), modified: None, path: None }),
    );
    h.settle();
    let video = h.app.doc.as_ref().unwrap().video.as_ref().unwrap();
    assert_eq!((video.view.first(), video.view.last(), video.current), (0, 1, 0));
}

#[test]
fn inclusive_measurements_use_all_frames_without_moving_the_view() {
    let mut doc = Doc::from_video(Video::open(sequence()).unwrap(), "clip.csq", None).unwrap();
    doc.seek(1).unwrap();
    let before = doc.image.celsius.clone();
    doc.measurement.mark_start(0);
    doc.measurement.mark_end(1);
    doc.measurement.begin(2, None);
    while doc.measurement.running() {
        doc.measurement.advance(doc.video.as_ref().unwrap());
    }
    let result = doc.measurement.result.as_ref().unwrap();
    let a = doc.video.as_ref().unwrap().frame(0).unwrap();
    let b = doc.video.as_ref().unwrap().frame(1).unwrap();
    assert_eq!(result.frames, 2);
    assert_eq!((result.start, result.end), (0, 1));
    assert_eq!(result.hottest.frame, 1);
    assert_eq!(result.coldest.frame, 0);
    assert!(
        (result.average_max - (a.stats.max.celsius as f64 + b.stats.max.celsius as f64) / 2.0)
            .abs()
            < 1e-6
    );
    let mean = a.celsius.iter().chain(&b.celsius).map(|&v| v as f64).sum::<f64>() / 96.0;
    assert!((result.mean - mean).abs() < 1e-6);
    assert_eq!(doc.image.celsius, before);
    assert_eq!(doc.video.as_ref().unwrap().current, 1);
    let mut params = doc.params();
    params.emissivity = 0.8;
    doc.set_params(params);
    assert!(doc.measurement.result.is_none(), "recalibration must invalidate old results");
    assert_eq!((doc.measurement.start, doc.measurement.end), (Some(0), Some(1)));
    doc.measurement.begin(2, Some(params));
    while doc.measurement.running() {
        doc.measurement.advance(doc.video.as_ref().unwrap());
    }
    let result = doc.measurement.result.as_ref().unwrap();
    let a = a.recompute(params);
    let b = b.recompute(params);
    assert!(result.edited_calibration);
    assert!(
        (result.average_max - (a.stats.max.celsius as f64 + b.stats.max.celsius as f64) / 2.0)
            .abs()
            < 1e-6
    );
    doc.measurement.mark_start(1);
    doc.measurement.mark_end(1);
    doc.measurement.begin(2, Some(params));
    while doc.measurement.running() {
        doc.measurement.advance(doc.video.as_ref().unwrap());
    }
    let result = doc.measurement.result.as_ref().unwrap();
    assert_eq!(result.frames, 1);
    assert_eq!(result.average_max, b.stats.max.celsius as f64);
}

fn corrupt_frame(mut bytes: Vec<u8>) -> Vec<u8> {
    let eoi = bytes.windows(2).rposition(|p| p == [0xff, 0xd9]).unwrap();
    bytes[eoi + 1] = 0;
    bytes
}

#[test]
fn unreadable_frames_are_skipped_and_averages_use_only_successful_frames() {
    let bad = corrupt_frame(frame(&[50000; 48], true));
    let good_a = frame(&[14000; 48], true);
    let good_b = frame(&[18000; 48], true);
    let bytes = [bad.clone(), good_a, bad.clone(), good_b, bad].concat();
    let video = Video::open(bytes).unwrap();
    let a = video.frame(1).unwrap();
    let b = video.frame(3).unwrap();
    let mut measurement = thermview::measure::Measurement::default();
    measurement.mark_start(0);
    measurement.mark_end(4);
    measurement.begin(video.len(), None);
    measurement.advance(&video);
    assert!(measurement.running(), "attempts remain bounded per UI update");
    let (done, total) = measurement.progress().unwrap();
    assert!((1..=4).contains(&done));
    assert_eq!(total, 5);
    while measurement.running() {
        measurement.advance(&video);
    }
    assert!(measurement.error.is_none());
    let result = measurement.result.as_ref().unwrap();
    assert_eq!((result.frames, result.selected_frames), (2, 5));
    assert_eq!(result.hottest.frame, 3);
    assert_eq!(result.coldest.frame, 1);
    assert_eq!(result.hottest_frames.len(), 2, "failed frames must not become candidates");
    assert_eq!(measurement.failures.iter().map(|f| f.frame).collect::<Vec<_>>(), vec![0, 2, 4]);
    assert!(measurement.failures[1].error.contains("CSQ frame 3"));
    assert_eq!(result.average_max, (a.stats.max.celsius as f64 + b.stats.max.celsius as f64) / 2.0);
    assert_eq!(result.average_min, (a.stats.min.celsius as f64 + b.stats.min.celsius as f64) / 2.0);
    let expected = a.celsius.iter().chain(&b.celsius).map(|&c| c as f64).sum::<f64>() / 96.0;
    assert!((result.mean - expected).abs() < 1e-9);
    measurement.mark_end(3);
    assert!(measurement.failures.is_empty());
    assert!(measurement.result.is_none());
}

#[test]
fn all_failed_frames_finish_without_fabricating_temperatures() {
    let bad = corrupt_frame(frame(&[14000; 48], true));
    let video = Video::open([bad.clone(), bad].concat()).unwrap();
    let mut measurement = thermview::measure::Measurement::default();
    measurement.mark_start(0);
    measurement.mark_end(1);
    measurement.begin(video.len(), None);
    while measurement.running() {
        measurement.advance(&video);
    }
    assert!(measurement.result.is_none());
    assert_eq!(measurement.failures.len(), 2);
    assert!(measurement.error.as_ref().unwrap().contains("No readable frames"));
    measurement.clear();
    assert!(measurement.failures.is_empty());
    assert!(measurement.error.is_none());
}

#[test]
fn measure_activity_marks_and_reports_the_selected_video_range() {
    let mut h = zoom_harness();
    h.click(h.find("MEASURE").unwrap().center());
    h.click(h.find("MARK START").unwrap().center());
    h.app.doc.as_mut().unwrap().seek(10).unwrap();
    h.settle();
    h.click(h.find("MARK END").unwrap().center());
    assert_eq!(
        (
            h.app.doc.as_ref().unwrap().measurement.start,
            h.app.doc.as_ref().unwrap().measurement.end
        ),
        (Some(0), Some(10))
    );
    assert!(h.find("I").is_some());
    assert!(h.find("O").is_some());
    h.click(h.find("ANALYSE RANGE").unwrap().center());
    for _ in 0..8 {
        h.frame(Vec::new());
    }
    let result = h.app.doc.as_ref().unwrap().measurement.result.as_ref().unwrap();
    assert_eq!(result.frames, 11);
    assert_eq!(result.hottest.frame, 10);
    assert!(h.find("HOTTEST").is_some());
    assert!(h.find("AVERAGE MAX").is_some());
    h.click(h.find("GO TO COLDEST").unwrap().center());
    assert_eq!(h.app.doc.as_ref().unwrap().video.as_ref().unwrap().current, 0);
    assert!(h.app.doc.as_ref().unwrap().measurement.result.is_some());
    std::fs::write(
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("csq-measure.png"),
        h.png(),
    )
    .unwrap();
    h.click(h.find("RESET").unwrap().center());
    assert_eq!(
        (
            h.app.doc.as_ref().unwrap().measurement.start,
            h.app.doc.as_ref().unwrap().measurement.end
        ),
        (None, None)
    );
    assert!(h.app.doc.as_ref().unwrap().measurement.result.is_none());
}

#[test]
fn i_and_o_shortcuts_mark_visible_frames_and_ignore_text_fields() {
    let mut h = zoom_harness();
    // A focused toolbar button must not prevent marker shortcuts.
    h.click(h.find("+").unwrap().center());
    h.frame(vec![egui::Event::Key {
        key: egui::Key::I,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    h.frame(vec![egui::Event::Key {
        key: egui::Key::I,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert_eq!(h.app.doc.as_ref().unwrap().measurement.start, Some(0));
    assert!(h.find("MARK END").is_some(), "shortcut opens Measure");
    h.app.doc.as_mut().unwrap().seek(8).unwrap();
    h.frame(vec![egui::Event::Key {
        key: egui::Key::O,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    h.frame(vec![egui::Event::Key {
        key: egui::Key::O,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert_eq!(h.app.doc.as_ref().unwrap().measurement.end, Some(8));
    h.app.doc.as_mut().unwrap().add_point(1, 1);
    h.settle();
    let name = h
        .texts()
        .into_iter()
        .find(|(text, rect)| text == "1" && rect.left() > 1200.0 && rect.top() < 450.0)
        .expect("point name in the right panel")
        .1;
    h.click(name.center());
    // The point-name text editor owns typed I/O rather than marking the range.
    assert!(h.ctx.text_edit_focused());
    h.app.doc.as_mut().unwrap().seek(9).unwrap();
    h.frame(vec![egui::Event::Key {
        key: egui::Key::I,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert_eq!(h.app.doc.as_ref().unwrap().measurement.start, Some(0));
}

#[test]
fn range_analysis_can_be_cancelled_and_new_files_clear_marks() {
    let mut h = zoom_harness();
    h.click(h.find("MEASURE").unwrap().center());
    h.click(h.find("MARK START").unwrap().center());
    h.app.doc.as_mut().unwrap().seek(100).unwrap();
    h.settle();
    h.click(h.find("MARK END").unwrap().center());
    h.click(h.find("ANALYSE RANGE").unwrap().center());
    assert!(h.app.doc.as_ref().unwrap().measurement.running());
    h.click(h.find("CANCEL").unwrap().center());
    assert!(!h.app.doc.as_ref().unwrap().measurement.running());
    assert!(h.app.doc.as_ref().unwrap().measurement.result.is_none());
    assert_eq!(h.app.doc.as_ref().unwrap().measurement.end, Some(100));
    h.app.load(
        &h.ctx.clone(),
        Ok(Loaded { name: "new.csq".into(), bytes: sequence(), modified: None, path: None }),
    );
    h.settle();
    assert_eq!(
        (
            h.app.doc.as_ref().unwrap().measurement.start,
            h.app.doc.as_ref().unwrap().measurement.end
        ),
        (None, None)
    );
}

#[test]
#[ignore = "set THERMVIEW_TEST_CSQ to the local recording; measures around frame 11148"]
fn real_recording_range_measurement_crosses_the_padded_frame() {
    let path = std::env::var("THERMVIEW_TEST_CSQ").unwrap();
    let mut doc =
        Doc::from_video(Video::open(std::fs::read(path).unwrap()).unwrap(), "recording.csq", None)
            .unwrap();
    doc.measurement.mark_start(11145);
    doc.measurement.mark_end(11149);
    doc.measurement.begin(doc.video.as_ref().unwrap().len(), None);
    while doc.measurement.running() {
        doc.measurement.advance(doc.video.as_ref().unwrap());
    }
    assert!(doc.measurement.error.is_none(), "{:?}", doc.measurement.error);
    let result = doc.measurement.result.as_ref().unwrap();
    assert_eq!(result.frames, 5);
    assert!((11145..=11149).contains(&result.hottest.frame));
    println!(
        "Frames {}–{} inclusive: max {:.2} C on frame {}, average max {:.2} C, min {:.2} C, mean {:.2} C",
        result.start + 1,
        result.end + 1,
        result.hottest.spot.celsius,
        result.hottest.frame + 1,
        result.average_max,
        result.coldest.spot.celsius,
        result.mean
    );
    assert_eq!(doc.video.as_ref().unwrap().current, 0);
}

#[test]
fn next_hottest_visits_each_ranked_frame_and_go_to_hottest_restarts() {
    let mut h = zoom_harness();
    h.click(h.find("MEASURE").unwrap().center());
    h.click(h.find("MARK START").unwrap().center());
    h.app.doc.as_mut().unwrap().seek(10).unwrap();
    h.settle();
    h.click(h.find("MARK END").unwrap().center());
    h.click(h.find("ANALYSE RANGE").unwrap().center());
    for _ in 0..8 {
        h.frame(Vec::new());
    }
    let go = h.find("GO TO HOTTEST").unwrap();
    let next = h.find("NEXT HOTTEST").unwrap();
    assert!((go.center().y - next.center().y).abs() < 1.0, "buttons must sit side by side");
    assert!(next.left() > go.right());
    h.click(go.center());
    assert_eq!(h.app.doc.as_ref().unwrap().video.as_ref().unwrap().current, 10);
    for expected in (0..10).rev() {
        h.click(h.find("NEXT HOTTEST").unwrap().center());
        let doc = h.app.doc.as_ref().unwrap();
        assert_eq!(doc.video.as_ref().unwrap().current, expected);
        let result = doc.measurement.result.as_ref().unwrap();
        assert_eq!(
            result.hottest.frame, 10,
            "browsing candidates must preserve the original maximum"
        );
        assert_eq!(result.hottest_frames[doc.measurement.hottest_rank].frame, expected);
    }
    assert!(h.find("11 of 11").is_some());
    h.click(h.find("NEXT HOTTEST").unwrap().center());
    assert_eq!(
        h.app.doc.as_ref().unwrap().video.as_ref().unwrap().current,
        0,
        "the list must stop at its end"
    );
    h.click(h.find("GO TO HOTTEST").unwrap().center());
    assert_eq!(h.app.doc.as_ref().unwrap().video.as_ref().unwrap().current, 10);
    assert_eq!(h.app.doc.as_ref().unwrap().measurement.hottest_rank, 0);
    h.click(h.find("NEXT HOTTEST").unwrap().center());
    assert_eq!(h.app.doc.as_ref().unwrap().video.as_ref().unwrap().current, 9);
    assert!(h.find("2 of 11").is_some());
    std::fs::write(
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("csq-next-hottest.png"),
        h.png(),
    )
    .unwrap();
    h.click(h.find("MARK END").unwrap().center());
    assert!(h.app.doc.as_ref().unwrap().measurement.result.is_none());
    assert_eq!(h.app.doc.as_ref().unwrap().measurement.hottest_rank, 0);
}

#[test]
fn failures_box_reports_skipped_frames_and_partial_coverage() {
    let good = frame(&[14000; 48], true);
    let bad = corrupt_frame(frame(&[50000; 48], true));
    let bytes = [good.clone(), bad.clone(), good, bad].concat();
    let mut h = Harness::new(1500.0, 1550.0);
    h.settle();
    h.app.load(
        &h.ctx.clone(),
        Ok(Loaded { name: "failures.csq".into(), bytes, modified: None, path: None }),
    );
    h.settle();
    h.click(h.find("MEASURE").unwrap().center());
    let measurement = &mut h.app.doc.as_mut().unwrap().measurement;
    measurement.mark_start(0);
    measurement.mark_end(3);
    h.settle();
    h.click(h.find("ANALYSE RANGE").unwrap().center());
    h.settle();
    assert!(h.find("2 / 4 frames").is_some());
    assert!(h.find("FAILURES").is_some());
    assert!(h.texts().iter().any(|(t, _)| t.contains("Partial coverage")));
    assert!(h.texts().iter().any(|(t, _)| t.starts_with("Frame 2:")));
    assert!(h.texts().iter().any(|(t, _)| t.starts_with("Frame 4:")));
    std::fs::write(
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("csq-failures.png"),
        h.png(),
    )
    .unwrap();
    h.click(h.find("GO TO HOTTEST").unwrap().center());
    assert_eq!(h.app.doc.as_ref().unwrap().video.as_ref().unwrap().current, 0);
    h.click(h.find("NEXT HOTTEST").unwrap().center());
    assert_eq!(h.app.doc.as_ref().unwrap().video.as_ref().unwrap().current, 2);
    h.click(h.find("RESET").unwrap().center());
    assert!(h.app.doc.as_ref().unwrap().measurement.failures.is_empty());
}

#[test]
#[ignore = "set THERMVIEW_TEST_CSQ; checks analysis around frame 5527"]
fn real_recording_measurement_continues_past_frame_5527() {
    let path = std::env::var("THERMVIEW_TEST_CSQ").unwrap();
    let video = Video::open(std::fs::read(path).unwrap()).unwrap();
    let mut measurement = thermview::measure::Measurement::default();
    measurement.mark_start(5524);
    measurement.mark_end(5528);
    measurement.begin(video.len(), None);
    while measurement.running() {
        measurement.advance(&video);
    }
    assert!(measurement.error.is_none(), "{:?}", measurement.error);
    let result = measurement.result.as_ref().unwrap();
    assert_eq!(result.selected_frames, 5);
    assert_eq!(result.frames + measurement.failures.len(), 5);
    assert!(
        result.hottest_frames.iter().any(|p| p.frame == 5528),
        "analysis must reach the frame after the reported error"
    );
    println!(
        "Frames 5525–5529: {} analysed, {} skipped",
        result.frames,
        measurement.failures.len()
    );
    for failure in &measurement.failures {
        println!("Skipped frame {}: {}", failure.frame + 1, failure.error);
    }
}

#[test]
fn frames_without_valid_temperatures_are_reported_and_skipped() {
    let good = frame(&[14000; 48], true);
    let invalid = frame(&[0; 48], true);
    let video = Video::open([good.clone(), invalid, good].concat()).unwrap();
    let mut measurement = thermview::measure::Measurement::default();
    measurement.mark_start(0);
    measurement.mark_end(2);
    measurement.begin(video.len(), None);
    while measurement.running() {
        measurement.advance(&video);
    }
    assert!(measurement.error.is_none());
    assert_eq!(measurement.result.as_ref().unwrap().frames, 2);
    assert_eq!(measurement.failures.len(), 1);
    assert_eq!(measurement.failures[0].frame, 1);
    assert!(measurement.failures[0].error.contains("no valid temperature"));
}
