use std::path::Path;
use thermview::image::Format;

fn sample(name: &str) -> Option<Vec<u8>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-samples").join(name);
    let data = std::fs::read(&path).ok();
    if data.is_none() {
        eprintln!("skipping, {} is not checked out", path.display());
    }
    data
}

fn close(a: f32, b: f64) -> bool {
    (a as f64 - b).abs() < 1e-3
}

#[test]
fn flir_e40_matches_the_typescript_parser() {
    let Some(data) = sample("flir_rjpeg_example.jpg") else { return };
    let img = thermview::parse(&data, Some("flir_rjpeg_example.jpg")).unwrap();
    assert_eq!((img.format, img.width, img.height), (Format::FlirJpeg, 240, 320));
    assert_eq!((img.stats.min.x, img.stats.min.y), (193, 45));
    assert_eq!((img.stats.max.x, img.stats.max.y), (99, 215));
    assert!(close(img.stats.min.celsius, 25.948277));
    assert!(close(img.stats.max.celsius, 62.320269));
    assert!(close(img.celsius[0], 26.175583));
    assert!(close(img.celsius[160 * 240 + 120], 30.500334));
    assert!(close(img.stats.cdf[512], 0.9434505));
    let planck = img.radiometry.as_ref().unwrap().planck;
    assert_eq!(planck.o, -1143.0);
    assert!((planck.r2 - 0.012332781).abs() < 1e-9);
    assert_eq!(img.camera.captured.as_deref(), Some("2017:09:08 16:04:36"));
    assert!((img.camera.latitude.unwrap() - 49.0107).abs() < 1e-4);
}

#[test]
fn flir_ax8_single_chunk_app1_reads_exiftools_planck_constants() {
    let Some(data) = sample("flir_ax8.jpg") else { return };
    let img = thermview::parse(&data, None).unwrap();
    assert_eq!((img.width, img.height), (80, 60));
    let planck = img.radiometry.as_ref().unwrap().planck;
    assert_eq!(planck.o, -7142.0);
    assert!((planck.r2 - 0.014294867).abs() < 1e-9);
    assert!((planck.r1 - 16951.797).abs() < 1e-3);
    assert!(close(img.celsius[0], 24.791489));
    assert_eq!(img.camera.model.as_deref(), Some("FLIR AX8"));
}

#[test]
fn recalibrating_moves_temperatures_and_restoring_brings_them_back() {
    let Some(data) = sample("flir_rjpeg_example.jpg") else { return };
    let img = thermview::parse(&data, None).unwrap();
    let low = img.recompute(thermview::image::Params { emissivity: 0.5, ..img.params });
    assert!(low.stats.max.celsius > img.stats.max.celsius + 10.0);
    let back = low.recompute(img.params);
    assert!((back.celsius[1234] - img.celsius[1234]).abs() < 1e-4);
}

#[test]
fn dji_xt_s_is_refused_rather_than_misread() {
    let Some(data) = sample("dji_test.jpg") else { return };
    let err = thermview::parse(&data, None).unwrap_err().to_string();
    assert!(err.starts_with("DJI R-JPEG"), "{err}");
}
