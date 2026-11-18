# Linux file-picker startup

Checked on 18 November 2026, following a reported 5–10 second delay in 0.2.1
between pressing Open/Open folder and the native picker appearing.

## Finding and change

The Linux build used RFD's default XDG desktop portal backend. Both actions
therefore depended on a separate desktop service before the picker appeared.
The local portal logs contained repeated "Too many open files" errors and a
GNOME portal crash on 17 November. These are evidence of service problems, not a
precise measurement of where every reported delay occurred.

The Linux dependency now selects RFD's GTK 3 backend directly, with default
features disabled. This removes the direct XDG portal request/fallback path
from the normal Linux build. The application still uses its asynchronous native
picker, with existing thermal/all-file filters, initial directory, folder
selection, cancellation handling and duplicate-request protection. Windows and
macOS keep their existing native backends.

GTK can still choose a portal in an explicitly portal-forced or sandboxed
environment. This change targets the normal system-installed Linux application;
it is not a general promise about sandboxed distributions or network folders.

An additional startup safeguard removes GTK module overrides inherited from an
unrelated Snap application when ThermView itself runs outside that Snap. The
test environment inherited VS Code's Snap GTK modules, which caused a system
GTK ABI error before this cleanup. The safeguard runs as the first action in
main, before application threads or GTK initialization. It preserves overrides
that do not point into that Snap's installation/cache directories and does not
alter an actual Snap-installed ThermView's environment.

## Verification

Live tests exercised `NativePicker::open` and `NativePicker::poll`, using the
same startup environment cleanup as the desktop executable. A GTK callback in
the test process detected when its own chooser widget became mapped, cancelled
that chooser, and verified that ThermView received `Choice::Cancelled` and
released the picker. No other application's windows were controlled.

| Action | Time to mapped chooser window |
|---|---:|
| Open file | 128.7 ms |
| Open folder | 127.1 ms |

Each case ran in a fresh process on the local Ubuntu/GNOME X11 desktop, starting
in the repository directory. Detection used a 10 ms timer. These are local smoke
measurements, not cross-platform benchmarks. The old build's delay was reported
by the user; no controlled before/after timing of the old portal backend was
recorded. Wayland and KDE have not been runtime-tested for this change.

Further checks:

- Workspace tests pass, including file/folder choice, cancellation and path tests.
- The web target still checks successfully; it does not depend on GTK.
- Linux's resolved RFD features select `gtk3`, without the XDG portal feature.
- Debian shared-library analysis includes the GTK 3 runtime dependency.
- Release build dependencies and source-install instructions include GTK 3.

The temporary test observer and harness are retained under `/tmp/thermview-picker-*`
on this computer; they are not application dependencies. The live results are in
`/tmp/thermview-picker-live.log`, test results in `/tmp/thermview-picker-tests.log`,
and web-check output in `/tmp/thermview-picker-wasm.log`.

The installed application was not replaced and no tag, push or release was made.
This change is prepared for 0.2.2, together with simpler release downloads, pending
approval to publish.
