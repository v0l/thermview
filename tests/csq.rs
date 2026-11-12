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
