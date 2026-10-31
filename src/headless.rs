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
        let app = App::new(&ctx);
        Harness {
            ctx,
            app,
            textures: Textures::default(),
            screen: Rect::from_min_size(Pos2::ZERO, vec2(width, height)),
            last: None,
        }
    }

    pub fn frame(&mut self, events: Vec<Event>) {
        let mut raw = RawInput { screen_rect: Some(self.screen), events, ..Default::default() };
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
