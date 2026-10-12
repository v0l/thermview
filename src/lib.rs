pub mod app;
pub mod bytes;
pub mod colorbar;
pub mod doc;
pub mod exif;
pub mod export;
pub mod formats;
pub mod headless;
pub mod host;
pub mod image;
pub mod overlay;
pub mod palette;
pub mod planck;
pub mod raster;

pub use formats::parse;
pub use image::ThermalImage;

#[derive(Clone, Debug, PartialEq)]
pub struct Error(String);

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Error(message.into())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}
