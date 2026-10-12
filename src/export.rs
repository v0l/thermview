use crate::doc::Doc;
use crate::overlay;
use crate::raster::{Canvas, Textures, encode_png};
use egui::{Pos2, RawInput, Rect, ViewportId, vec2};

pub fn rgba(doc: &Doc) -> (usize, usize, Vec<u8>) {
    let base = doc.look.render(&doc.image);
    let mut canvas = Canvas::from_image(&base, doc.scale.max(1) as usize);
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(canvas.width as f32, canvas.height as f32));
    let ctx = egui::Context::default();
    egui_bench::install(&ctx);
    let mut textures = Textures::default();
    let mut last = None;
    for _ in 0..2 {
        let mut raw = RawInput { screen_rect: Some(screen), ..Default::default() };
        raw.viewports.entry(ViewportId::ROOT).or_default().native_pixels_per_point = Some(1.0);
        let out = ctx.run_ui(raw, |ui| overlay::paint(ui.painter(), screen, doc));
        textures.apply(&out.textures_delta);
        last = Some(out);
    }
    let out = last.expect("two passes ran");
    let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
    canvas.draw(&prims, &textures, out.pixels_per_point);
    (canvas.width, canvas.height, canvas.to_rgba8())
}

pub fn png(doc: &Doc) -> Vec<u8> {
    let (w, h, px) = rgba(doc);
    encode_png(w, h, &px)
}

pub fn file_name(doc: &Doc) -> String {
    let stem = doc.name.rsplit_once('.').map(|(s, _)| s).unwrap_or(&doc.name);
    let stem = if stem.is_empty() { "thermal-image" } else { stem };
    format!("{stem}-thermview.png")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{Format, ThermalImage};

    fn doc() -> Doc {
        let celsius: Vec<f32> = (0..40 * 30).map(|i| 20.0 + (i % 40) as f32 * 0.5).collect();
        let img = ThermalImage::new(Format::Hikmicro, 40, 30, celsius);
        Doc::new(img, "scene.jpg", None)
    }

    #[test]
    fn export_is_the_scaled_image_with_the_overlays_on_top() {
        let mut d = doc();
        d.scale = 4;
        d.overlays.info = false;
        d.overlays.time = false;
        d.overlays.extremes = false;
        let (w, h, plain) = rgba(&d);
        assert_eq!((w, h), (160, 120));
        let base = d.look.render(&d.image);
        let px =
            |buf: &[u8], x: usize, y: usize| buf[(y * w + x) * 4..(y * w + x) * 4 + 4].to_vec();
        assert_eq!(px(&plain, 157, 3), base.pixels[39].to_array().to_vec());

        d.add_point(20, 15);
        let (_, _, marked) = rgba(&d);
        let changed = plain.chunks(4).zip(marked.chunks(4)).filter(|(a, b)| a != b).count();
        assert!(changed > 200, "only {changed} pixels changed under a crosshair and label");
        assert_eq!(px(&marked, 2, 2), px(&plain, 2, 2));
    }

    #[test]
    fn export_names_follow_the_source() {
        assert_eq!(file_name(&doc()), "scene-thermview.png");
    }
}
