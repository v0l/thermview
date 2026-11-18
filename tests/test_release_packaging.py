"""Release contract checks: clear downloads, full platform coverage and integrity."""
import hashlib
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("prepare_release", ROOT / "packaging/prepare-release.py")
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseDownloads(unittest.TestCase):
    def setUp(self):
        self.workspace = tempfile.TemporaryDirectory()
        self.addCleanup(self.workspace.cleanup)
        self.root = Path(self.workspace.name)
        self.inputs = self.root / "builds"
        self.inputs.mkdir()
        self.output = self.root / "public"
        self.notes = self.root / "notes.md"
        self.notes.write_text("### Fixed\n- Faster native file picker.\n")
        for _, name, _ in release.installers("0.2.2"):
            (self.inputs / name).write_bytes(b"package fixture: " + name.encode())

    def prepare(self):
        release.prepare("0.2.2", "alfiethology/thermview", self.inputs, self.output, self.notes)

    def test_only_six_readable_installers_are_published(self):
        for name in ["old-portable.zip", "old-portable.tar.gz", "debug.txt", "installer.sha256"]:
            (self.inputs / name).write_text("CI-only content")
        self.prepare()
        self.assertEqual({p.name for p in self.output.iterdir()}, {
            "ThermView-0.2.2-macOS-Apple-Silicon.dmg",
            "ThermView-0.2.2-macOS-Intel.dmg",
            "ThermView-0.2.2-Windows-11-x64.msi",
            "ThermView-0.2.2-Linux-Debian-Ubuntu-x64.deb",
            "ThermView-0.2.2-Linux-Debian-Ubuntu-ARM64.deb",
            "ThermView-0.2.2-Linux-Arch-x64.pkg.tar.zst",
        })
        notes = self.notes.read_text()
        self.assertIn("archive/refs/tags/v0.2.2.zip", notes)
        self.assertIn("<summary>Verify downloads (SHA-256)</summary>", notes)
        self.assertIn("Faster native file picker.", notes)
        for asset in self.output.iterdir():
            self.assertIn("/releases/download/v0.2.2/" + asset.name, notes)
            self.assertIn(hashlib.sha256(asset.read_bytes()).hexdigest() + "  " + asset.name, notes)

    def test_missing_arch_installer_prevents_partial_publication(self):
        (self.inputs / "thermview-0.2.2-1-x86_64.pkg.tar.zst").unlink()
        original = self.notes.read_text()
        with self.assertRaisesRegex(ValueError, "Missing or empty installer"):
            self.prepare()
        self.assertFalse(self.output.exists())
        self.assertEqual(self.notes.read_text(), original)

    def test_existing_output_is_not_overwritten(self):
        self.output.mkdir()
        keep = self.output / "existing.txt"
        keep.write_text("keep")
        with self.assertRaises(FileExistsError):
            self.prepare()
        self.assertEqual(keep.read_text(), "keep")

    def test_empty_installers_and_empty_notes_are_rejected(self):
        first = next(self.inputs.iterdir())
        first.write_bytes(b"")
        with self.assertRaisesRegex(ValueError, "empty installer"):
            self.prepare()
        first.write_bytes(b"package")
        self.notes.write_text("")
        with self.assertRaisesRegex(ValueError, "notes are empty"):
            self.prepare()


class ArchPackageLayout(unittest.TestCase):
    def test_package_installs_binary_desktop_entry_mime_and_icons(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "src/thermview-0.2.2"
            files = ["target/release/thermview", "packaging/linux/thermview.desktop",
                     "packaging/linux/thermview.xml", "web/public/favicon.svg",
                     "web/public/icon-192.png", "LICENSE", "README.md"]
            for name in files:
                path = source / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("fixture for " + name)
            build = root / "PKGBUILD"
            build.write_text((ROOT / "packaging/arch/PKGBUILD.in").read_text()
                             .replace("__VERSION__", "0.2.2")
                             .replace("__SOURCE_SHA256__", "0" * 64))
            subprocess.run([
                "bash", "-c", 'source "$1"; srcdir="$2"; pkgdir="$3"; package',
                "bash", str(build), str(root / "src"), str(root / "pkg"),
            ], check=True)
            self.assertEqual((root / "pkg/usr/bin/thermview").stat().st_mode & 0o777, 0o755)
            expected = ["usr/share/applications/thermview.desktop",
                        "usr/share/mime/packages/thermview.xml",
                        "usr/share/icons/hicolor/scalable/apps/thermview.svg",
                        "usr/share/icons/hicolor/192x192/apps/thermview.png",
                        "usr/share/licenses/thermview/LICENSE",
                        "usr/share/doc/thermview/README.md"]
            for name in expected:
                self.assertTrue((root / "pkg" / name).is_file(), name)


if __name__ == "__main__":
    unittest.main()
