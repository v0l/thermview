#[cfg(target_arch = "wasm32")]
mod start {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            let document = web_sys::window().and_then(|w| w.document()).expect("no document");
            let canvas = document
                .get_element_by_id("thermview")
                .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
                .expect("the page has no <canvas id=\"thermview\">");
            let started = eframe::WebRunner::new()
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(|cc| Ok(Box::new(thermview::app::App::new(&cc.egui_ctx)))),
                )
                .await;
            if let Some(loading) = document.get_element_by_id("loading") {
                loading.remove();
            }
            if let Err(error) = started {
                web_sys::console::error_1(&error);
            }
        });
    }
}
