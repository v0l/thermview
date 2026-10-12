use egui::epaint::{ClippedPrimitive, Primitive, Vertex};
use egui::{ColorImage, TextureId, TexturesDelta};
use std::collections::HashMap;

#[derive(Default)]
pub struct Textures {
    map: HashMap<TextureId, ColorImage>,
}

impl Textures {
    pub fn apply(&mut self, delta: &TexturesDelta) {
        for (id, d) in &delta.set {
            let egui::ImageData::Color(img) = &d.image;
            match d.pos {
                None => {
                    self.map.insert(*id, (**img).clone());
                }
                Some([x0, y0]) => {
                    let Some(dst) = self.map.get_mut(id) else { continue };
                    let [w, h] = img.size;
                    for y in 0..h {
                        for x in 0..w {
                            let (dx, dy) = (x0 + x, y0 + y);
                            if dx < dst.size[0] && dy < dst.size[1] {
                                dst.pixels[dy * dst.size[0] + dx] = img.pixels[y * w + x];
                            }
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.map.remove(id);
        }
    }
}

pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<[f32; 4]>,
}

impl Canvas {
    pub fn blank(width: usize, height: usize, fill: egui::Color32) -> Canvas {
        let px = fill.to_array().map(|v| v as f32 / 255.0);
        Canvas { width, height, rgba: vec![px; width * height] }
    }

    pub fn from_image(image: &ColorImage, scale: usize) -> Canvas {
        let [w, h] = image.size;
        let (width, height) = (w * scale, h * scale);
        let mut rgba = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let c = image.pixels[(y / scale) * w + x / scale];
                rgba.push(c.to_array().map(|v| v as f32 / 255.0));
            }
        }
        Canvas { width, height, rgba }
    }

    pub fn draw(&mut self, prims: &[ClippedPrimitive], tex: &Textures, ppp: f32) {
        for cp in prims {
            let Primitive::Mesh(mesh) = &cp.primitive else { continue };
            let clip = cp.clip_rect;
            let bounds = [
                (clip.min.x * ppp).floor().max(0.0) as i64,
                (clip.min.y * ppp).floor().max(0.0) as i64,
                ((clip.max.x * ppp).ceil() as i64).min(self.width as i64),
                ((clip.max.y * ppp).ceil() as i64).min(self.height as i64),
            ];
            if bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
                continue;
            }
            let texture = tex.map.get(&mesh.texture_id);
            for tri in mesh.indices.chunks_exact(3) {
                let v = [0, 1, 2].map(|k| &mesh.vertices[tri[k] as usize]);
                self.triangle(v, texture, ppp, bounds);
            }
        }
    }

    fn triangle(&mut self, v: [&Vertex; 3], tex: Option<&ColorImage>, ppp: f32, clip: [i64; 4]) {
        let p = v.map(|x| [x.pos.x * ppp, x.pos.y * ppp]);
        let area =
            (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
        if area.abs() < 1e-8 {
            return;
        }
        let xs = p.map(|q| q[0]);
        let ys = p.map(|q| q[1]);
        let x0 = (xs.iter().copied().fold(f32::MAX, f32::min).floor() as i64).max(clip[0]);
        let y0 = (ys.iter().copied().fold(f32::MAX, f32::min).floor() as i64).max(clip[1]);
        let x1 = (xs.iter().copied().fold(f32::MIN, f32::max).ceil() as i64).min(clip[2]);
        let y1 = (ys.iter().copied().fold(f32::MIN, f32::max).ceil() as i64).min(clip[3]);
        let col = v.map(|x| x.color.to_array().map(|c| c as f32 / 255.0));
        let edge = |a: [f32; 2], b: [f32; 2], x: f32, y: f32| {
            (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])
        };
        let inv = 1.0 / area;
        let top_left = |a: [f32; 2], b: [f32; 2]| {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let (dx, dy) = if area > 0.0 { (dx, dy) } else { (-dx, -dy) };
            (dy == 0.0 && dx > 0.0) || dy < 0.0
        };
        let owns = [top_left(p[1], p[2]), top_left(p[2], p[0]), top_left(p[0], p[1])];
        for y in y0..y1 {
            let py = y as f32 + 0.5;
            for x in x0..x1 {
                let px = x as f32 + 0.5;
                let e =
                    [edge(p[1], p[2], px, py), edge(p[2], p[0], px, py), edge(p[0], p[1], px, py)];
                let inside = (0..3).all(|k| {
                    let w = e[k] * area.signum();
                    w > 0.0 || (w == 0.0 && owns[k])
                });
                if !inside {
                    continue;
                }
                let (w0, w1) = (e[0] * inv, e[1] * inv);
                let w2 = 1.0 - w0 - w1;
                let mut c = [0.0f32; 4];
                for k in 0..4 {
                    c[k] = col[0][k] * w0 + col[1][k] * w1 + col[2][k] * w2;
                }
                if let Some(t) = tex {
                    let u = v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2;
                    let s = v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2;
                    let texel = sample(t, u, s);
                    for k in 0..4 {
                        c[k] *= texel[k];
                    }
                }
                let d = &mut self.rgba[y as usize * self.width + x as usize];
                let keep = 1.0 - c[3];
                for k in 0..4 {
                    d[k] = c[k] + d[k] * keep;
                }
            }
        }
    }

    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.width * self.height * 4);
        for px in &self.rgba {
            let a = px[3].clamp(0.0, 1.0);
            let un = |c: f32| if a > 0.0 { (c / a).clamp(0.0, 1.0) } else { 0.0 };
            out.extend([un(px[0]), un(px[1]), un(px[2]), a].map(|c| (c * 255.0 + 0.5) as u8));
        }
        out
    }
}

fn sample(t: &ColorImage, u: f32, v: f32) -> [f32; 4] {
    let [w, h] = t.size;
    let x = (u * w as f32 - 0.5).clamp(0.0, w as f32 - 1.0);
    let y = (v * h as f32 - 0.5).clamp(0.0, h as f32 - 1.0);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let px = |x: usize, y: usize| t.pixels[y * w + x].to_array().map(|c| c as f32 / 255.0);
    let (a, b, c, d) = (px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1));
    std::array::from_fn(|k| {
        let top = a[k] + (b[k] - a[k]) * fx;
        let bottom = c[k] + (d[k] - c[k]) * fx;
        top + (bottom - top) * fy
    })
}

pub fn encode_png(width: usize, height: usize, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, width as u32, height as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().expect("png header");
    w.write_image_data(rgba).expect("png data");
    w.finish().expect("png finish");
    out
}
