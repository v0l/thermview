//! Native startup setup and persistent diagnostics, including console-free launches.
use std::io::Write;
use std::path::PathBuf;

/// Remove GTK module overrides inherited from an unrelated Snap application.
/// A system-installed binary must load modules matching its system GTK library.
/// Overrides that do not reference those Snap directories are preserved.
///
/// # Safety
/// Call only at process startup, before creating threads or initializing GTK.
#[cfg(target_os = "linux")]
pub unsafe fn prepare_system_gtk() {
    let Some(snap) = std::env::var_os("SNAP").map(PathBuf::from) else { return };
    let Ok(executable) = std::env::current_exe() else { return };
    if executable.starts_with(&snap) {
        return; // Do not change the environment of an actual Snap build.
    }
    let mut roots = vec![snap];
    roots.extend(
        ["SNAP_USER_COMMON", "SNAP_USER_DATA"]
            .into_iter()
            .filter_map(|key| std::env::var_os(key).map(PathBuf::from)),
    );
    for key in [
        "GTK_PATH",
        "GTK_EXE_PREFIX",
        "GTK_IM_MODULE_FILE",
        "GIO_MODULE_DIR",
        "GIO_EXTRA_MODULES",
        "GDK_PIXBUF_MODULE_FILE",
        "GDK_PIXBUF_MODULEDIR",
    ] {
        if std::env::var_os(key).is_some_and(|value| foreign_gtk_path(&value, &roots)) {
            // SAFETY: the caller guarantees single-threaded process startup.
            unsafe { std::env::remove_var(key) };
        }
    }
}

#[cfg(target_os = "linux")]
fn foreign_gtk_path(value: &std::ffi::OsStr, roots: &[PathBuf]) -> bool {
    std::env::split_paths(value).any(|path| roots.iter().any(|root| path.starts_with(root)))
}

#[cfg(all(test, target_os = "linux"))]
mod gtk_environment_tests {
    use super::*;

    #[test]
    fn identifies_foreign_snap_modules_without_matching_normal_user_overrides() {
        let roots =
            vec![PathBuf::from("/snap/code/280"), PathBuf::from("/home/example/snap/code/common")];
        for value in [
            "/snap/code/280/usr/lib/gtk-3.0",
            "/home/example/snap/code/common/.cache/gio-modules",
            "/usr/lib/gtk-3.0:/snap/code/280/usr/lib/gtk-3.0",
        ] {
            assert!(foreign_gtk_path(value.as_ref(), &roots));
        }
        for value in [
            "/usr/lib/gtk-3.0",
            "/home/example/custom-gtk",
            "/snap/code/2800/usr/lib/gtk-3.0",
            "/home/example/snap/code/common-other/modules",
        ] {
            assert!(!foreign_gtk_path(value.as_ref(), &roots));
        }
    }
}

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
