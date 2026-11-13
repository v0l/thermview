//! Persistent native startup diagnostics, including launches without a console.
use std::io::Write;
use std::path::PathBuf;

pub fn log_path() -> PathBuf {
    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let root = std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Logs"));
    #[cfg(not(any(windows, target_os = "macos")))]
    let root = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")));
    root.unwrap_or_else(std::env::temp_dir).join("ThermView/startup.log")
}

pub fn log(message: &str) -> Option<PathBuf> {
    let path = log_path();
    std::fs::create_dir_all(path.parent()?).ok()?;
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path).ok()?;
    writeln!(
        file,
        "{} thermview {} [{}] {message}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS
    )
    .ok()?;
    Some(path)
}

pub fn install_panic_log() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log(&format!("Panic: {info}\n{}", std::backtrace::Backtrace::force_capture()));
        previous(info);
    }));
}

pub fn report_error(message: &str, gui: bool) {
    let path = log(message);
    eprintln!("thermview: {message}");
    if let Some(path) = &path {
        eprintln!("Details: {}", path.display());
    }
    #[cfg(windows)]
    if gui {
        let details = match path {
            Some(path) => format!(
                "ThermView could not start or continue running.\n\n{message}\n\nDiagnostic log:\n{}",
                path.display()
            ),
            None => format!(
                "ThermView could not start or continue running.\n\n{message}\n\nThe diagnostic log could not be written."
            ),
        };
        rfd::MessageDialog::new()
            .set_title("ThermView error")
            .set_description(details)
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
    #[cfg(not(windows))]
    let _ = gui;
}
