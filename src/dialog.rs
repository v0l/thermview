//! Native desktop dialogs, polled by egui without blocking its event loop.
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    File,
    Folder,
}
#[derive(Debug, PartialEq)]
pub enum Choice {
    File(PathBuf),
    Folder(PathBuf),
    Cancelled,
}
pub type DialogFuture = Pin<Box<dyn Future<Output = Option<PathBuf>>>>;

pub struct NativePicker {
    request: Option<(Kind, PathBuf)>,
    pending: Option<(Kind, DialogFuture)>,
    backend: Option<fn(Kind, PathBuf) -> DialogFuture>,
    parent: Option<rfd::AsyncFileDialog>,
}

impl Default for NativePicker {
    fn default() -> Self {
        Self { request: None, pending: None, backend: None, parent: None }
    }
}

impl NativePicker {
    /// A replaceable backend also lets headless tests drive the real open flow
    /// without presenting an operating-system dialog.
    pub fn with_backend(backend: fn(Kind, PathBuf) -> DialogFuture) -> Self {
        Self { request: None, pending: None, backend: Some(backend), parent: None }
    }
    pub fn set_parent(&mut self, window: &eframe::CreationContext<'_>) {
        self.parent = Some(rfd::AsyncFileDialog::new().set_parent(window));
    }
    pub fn busy(&self) -> bool {
        self.request.is_some() || self.pending.is_some()
    }
    pub fn open(&mut self, kind: Kind, directory: PathBuf, ctx: &egui::Context) {
        if !self.busy() {
            self.request = Some((kind, directory));
            ctx.request_repaint();
        }
    }
    pub fn poll(&mut self, ctx: &egui::Context) -> Option<Choice> {
        if let Some((kind, directory)) = self.request.take() {
            let future = match self.backend {
                Some(backend) => backend(kind, directory),
                None => native_dialog(kind, directory, self.parent.clone()),
            };
            self.pending = Some((kind, future));
        }
        let (kind, future) = self.pending.as_mut()?;
        let kind = *kind;
        let waker = Waker::from(Arc::new(Repaint(ctx.clone())));
        match future.as_mut().poll(&mut Context::from_waker(&waker)) {
            Poll::Pending => None,
            Poll::Ready(path) => {
                self.pending = None;
                Some(match (kind, path) {
                    (Kind::File, Some(path)) => Choice::File(path),
                    (Kind::Folder, Some(path)) => Choice::Folder(path),
                    (_, None) => Choice::Cancelled,
                })
            }
        }
    }
}

struct Repaint(egui::Context);
impl Wake for Repaint {
    fn wake(self: Arc<Self>) {
        self.0.request_repaint();
    }
}

fn native_dialog(
    kind: Kind,
    directory: PathBuf,
    parent: Option<rfd::AsyncFileDialog>,
) -> DialogFuture {
    Box::pin(async move {
        let dialog = parent.unwrap_or_default().set_directory(directory);
        let selected = match kind {
            Kind::File => {
                dialog
                    .set_title("Open a thermal image or video")
                    .add_filter(
                        "Thermal images and videos",
                        &[
                            "csq", "CSQ", "irg", "IRG", "jpg", "JPG", "jpeg", "JPEG", "img", "IMG",
                            "seq", "SEQ",
                        ],
                    )
                    .add_filter("All files", &["*"])
                    .pick_file()
                    .await
            }
            Kind::Folder => dialog.set_title("Open a folder").pick_folder().await,
        };
        selected.map(|file| file.path().to_path_buf())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_selection_preserves_paths_and_cancellation_releases_the_picker() {
        let ctx = egui::Context::default();
        let mut picker = NativePicker::with_backend(|_, dir| {
            Box::pin(std::future::ready(Some(dir.join("thermal ü with spaces.csq"))))
        });
        picker.open(Kind::File, PathBuf::from("folder"), &ctx);
        picker.open(Kind::Folder, PathBuf::from("ignored"), &ctx);
        assert!(picker.busy());
        assert_eq!(
            picker.poll(&ctx),
            Some(Choice::File(PathBuf::from("folder/thermal ü with spaces.csq")))
        );
        assert!(!picker.busy());
        picker.open(Kind::Folder, PathBuf::from("folder"), &ctx);
        assert!(matches!(picker.poll(&ctx), Some(Choice::Folder(_))));
        let mut picker = NativePicker::with_backend(|_, _| Box::pin(std::future::ready(None)));
        picker.open(Kind::File, PathBuf::new(), &ctx);
        assert_eq!(picker.poll(&ctx), Some(Choice::Cancelled));
        assert!(!picker.busy());
    }
}
