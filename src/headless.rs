use crate::app::App;
use crate::raster::{Canvas, Textures, encode_png};
use egui::{Event, FullOutput, Modifiers, PointerButton, Pos2, RawInput, Rect, ViewportId, vec2};

pub struct Harness {
    pub ctx: egui::Context,
    pub app: App,
    textures: Textures,
    screen: Rect,
    last: Option<FullOutput>,
}

impl Harness {
    pub fn new(width: f32, height: f32) -> Harness {
        let ctx = egui::Context::default();
        #[allow(unused_mut)]
        let mut app = App::new(&ctx);
        #[cfg(not(target_arch = "wasm32"))]
        {
            app.file_dialog =
                crate::dialog::NativePicker::with_backend(|_, _| Box::pin(std::future::pending()));
        }
        Harness {
            ctx,
            app,
            textures: Textures::default(),
            screen: Rect::from_min_size(Pos2::ZERO, vec2(width, height)),
            last: None,
        }
    }

    pub fn frame(&mut self, events: Vec<Event>) {
        self.frame_with_time(events, None);
    }

    pub fn frame_at(&mut self, events: Vec<Event>, time: f64) {
        self.frame_with_time(events, Some(time));
    }

    fn frame_with_time(&mut self, events: Vec<Event>, time: Option<f64>) {
        let mut raw =
            RawInput { screen_rect: Some(self.screen), events, time, ..Default::default() };
        raw.viewports.entry(ViewportId::ROOT).or_default().native_pixels_per_point = Some(1.0);
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(raw, |ui| app.show(ui));
        self.textures.apply(&mut out.textures_delta);
        self.last = Some(out);
    }

    pub fn settle(&mut self) {
        for _ in 0..3 {
            self.frame(Vec::new());
        }
    }

    pub fn click(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![button(at, true)]);
        self.frame(vec![button(at, false)]);
        self.frame(Vec::new());
    }

    pub fn drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        for i in 1..=8 {
            let t = i as f32 / 8.0;
            self.frame(vec![Event::PointerMoved(from + (to - from) * t)]);
        }
        self.frame(vec![button(to, false)]);
        self.frame(Vec::new());
    }

    pub fn texts(&self) -> Vec<(String, Rect)> {
        let Some(out) = &self.last else { return Vec::new() };
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => {
                    Some((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2())))
                }
                _ => None,
            })
            .collect()
    }

    pub fn find(&self, text: &str) -> Option<Rect> {
        self.texts().into_iter().find(|(t, _)| t == text).map(|(_, r)| r)
    }

    pub fn thermal_rect(&self) -> Option<Rect> {
        self.last.as_ref()?.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
                Some(mesh.calc_bounds())
            }
            _ => None,
        })
    }

    pub fn text_colors(&self, text: &str) -> Vec<egui::Color32> {
        self.last
            .as_ref()
            .into_iter()
            .flat_map(|out| &out.shapes)
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(t) if t.galley.text() == text => {
                    Some(t.galley.job.sections.iter().map(|s| s.format.color))
                }
                _ => None,
            })
            .flatten()
            .collect()
    }

    pub fn png(&self) -> Vec<u8> {
        let out = self.last.as_ref().expect("a frame ran");
        let prims = self.ctx.tessellate(out.shapes.clone(), out.pixels_per_point);
        let (w, h) = (self.screen.width() as usize, self.screen.height() as usize);
        let mut canvas = Canvas::blank(w, h, egui_bench::theme::CHASSIS);
        canvas.draw(&prims, &self.textures, out.pixels_per_point);
        encode_png(w, h, &canvas.to_rgba8())
    }
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}
