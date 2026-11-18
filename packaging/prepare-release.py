#!/usr/bin/env python3
"""Stage the small public installer set; keep CI-only archives out of Releases."""
import argparse
import hashlib
from pathlib import Path
import re
import shutil


def installers(version):
    prefix = f"ThermView-{version}"
    return [
        ("macOS — Apple silicon (M-series)", "thermview-aarch64-apple-darwin.dmg",
         f"{prefix}-macOS-Apple-Silicon.dmg"),
        ("macOS — Intel", "thermview-x86_64-apple-darwin.dmg",
         f"{prefix}-macOS-Intel.dmg"),
        ("Windows 11 — Intel/AMD 64-bit", "thermview-x86_64-pc-windows-msvc.msi",
         f"{prefix}-Windows-11-x64.msi"),
        ("Linux — Debian/Ubuntu, Intel/AMD 64-bit", f"thermview_{version}-1_amd64.deb",
         f"{prefix}-Linux-Debian-Ubuntu-x64.deb"),
        ("Linux — Debian/Ubuntu, ARM64", f"thermview_{version}-1_arm64.deb",
         f"{prefix}-Linux-Debian-Ubuntu-ARM64.deb"),
        ("Linux — Arch, Intel/AMD 64-bit", f"thermview-{version}-1-x86_64.pkg.tar.zst",
         f"{prefix}-Linux-Arch-x64.pkg.tar.zst"),
    ]


def prepare(version, repo, source, output, notes):
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("Expected a stable version such as 0.2.2")
    if not re.fullmatch(r"[\w.-]+/[\w.-]+", repo):
        raise ValueError("Expected a GitHub owner/repository")
    assets = installers(version)
    # Check the complete set before creating any public assets or editing notes.
    for _, name, _ in assets:
        if not (source / name).is_file() or not (source / name).stat().st_size:
            raise ValueError(f"Missing or empty installer: {name}")
    original_notes = notes.read_text().strip()
    if not original_notes:
        raise ValueError("Release notes are empty")
    output.mkdir(parents=True, exist_ok=False)
    rows, checksums = [], []
    base = f"https://github.com/{repo}/releases/download/v{version}"
    for platform, name, public_name in assets:
        target = output / public_name
        shutil.copyfile(source / name, target)
        digest = hashlib.sha256()
        with target.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
        rows.append(f"| {platform} | [Download]({base}/{public_name}) |")
        checksums.append(f"{digest.hexdigest()}  {public_name}")
    rows.append(f"| Source code | [Download ZIP](https://github.com/{repo}/archive/refs/tags/v{version}.zip) |")
    notes.write_text(
        "## Downloads\n\nChoose the installer for your computer. "
        "Source code is for building the application yourself.\n\n"
        "| Computer | Installer |\n|---|---|\n" + "\n".join(rows)
        + "\n\nInstall Debian/Ubuntu packages with `sudo apt install ./FILENAME.deb`; "
        "install Arch packages with `sudo pacman -U ./FILENAME.pkg.tar.zst`.\n\n"
        + "## Changes\n\n" + original_notes
        + "\n\n<details>\n<summary>Verify downloads (SHA-256)</summary>\n\n```text\n"
        + "\n".join(checksums) + "\n```\n\n</details>\n"
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--notes", type=Path, required=True)
    args = parser.parse_args()
    prepare(args.version, args.repo, args.input, args.output, args.notes)


if __name__ == "__main__":
    main()
