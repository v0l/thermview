pub struct Loaded {
    pub name: String,
    pub bytes: Vec<u8>,
    pub modified: Option<String>,
    pub path: Option<std::path::PathBuf>,
}

pub const SUFFIXES: [&str; 10] =
    [".irg", ".IRG", ".jpg", ".JPG", ".jpeg", ".JPEG", ".img", ".IMG", ".seq", ".SEQ"];

pub fn dropped(file: &egui::DroppedFile) -> Result<Loaded, String> {
    if let Some(bytes) = &file.bytes {
        return Ok(Loaded {
            name: file.name.clone(),
            bytes: bytes.to_vec(),
            modified: file.last_modified.and_then(crate::doc::file_stamp),
            path: None,
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = &file.path {
        return read(path);
    }
    Err(format!("could not read {}", file.name))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read(path: &std::path::Path) -> Result<Loaded, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    Ok(Loaded {
        name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        bytes,
        modified: modified.and_then(crate::doc::file_stamp),
        path: Some(path.to_path_buf()),
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save(beside: Option<&std::path::Path>, name: &str, bytes: &[u8]) -> Result<String, String> {
    let dir = beside.and_then(|p| p.parent()).map(|p| p.to_path_buf()).unwrap_or_default();
    let target = dir.join(name);
    std::fs::write(&target, bytes).map_err(|e| format!("{}: {e}", target.display()))?;
    Ok(format!("wrote {}", target.display()))
}

#[cfg(target_arch = "wasm32")]
pub fn save(_beside: Option<&std::path::Path>, name: &str, bytes: &[u8]) -> Result<String, String> {
    use wasm_bindgen::JsCast;
    let fail = |e: wasm_bindgen::JsValue| format!("download failed: {e:?}");
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("image/png");
    let blob =
        web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(fail)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(fail)?;
    let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let a: web_sys::HtmlAnchorElement =
        document.create_element("a").map_err(fail)?.dyn_into().map_err(|_| "no anchor")?;
    a.set_href(&url);
    a.set_download(name);
    a.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(format!("downloaded {name}"))
}

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct Picker {
    slot: std::rc::Rc<std::cell::RefCell<Option<Loaded>>>,
}

#[cfg(target_arch = "wasm32")]
impl Picker {
    pub fn pick(&self, ctx: &egui::Context) {
        use wasm_bindgen::JsCast;
        use wasm_bindgen::prelude::*;
        let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
        let Ok(input) = document.create_element("input") else { return };
        let Ok(input) = input.dyn_into::<web_sys::HtmlInputElement>() else { return };
        input.set_type("file");
        input.set_accept(&SUFFIXES.iter().step_by(2).copied().collect::<Vec<_>>().join(","));
        let (slot, ctx, picked) = (self.slot.clone(), ctx.clone(), input.clone());
        let changed = Closure::<dyn FnMut()>::new(move || {
            let Some(file) = picked.files().and_then(|f| f.get(0)) else { return };
            let (slot, ctx) = (slot.clone(), ctx.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let Ok(buf) = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await
                else {
                    return;
                };
                let modified = chrono::DateTime::from_timestamp_millis(file.last_modified() as i64)
                    .map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string());
                *slot.borrow_mut() = Some(Loaded {
                    name: file.name(),
                    bytes: js_sys::Uint8Array::new(&buf).to_vec(),
                    modified,
                    path: None,
                });
                ctx.request_repaint();
            });
        });
        input.set_onchange(Some(changed.as_ref().unchecked_ref()));
        changed.forget();
        input.click();
    }

    pub fn take(&self) -> Option<Loaded> {
        self.slot.borrow_mut().take()
    }
}
