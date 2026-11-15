#!/usr/bin/env python3
"""Download exact upstream reference sources and a public thermal sample.

Only downloads/extracts data; does not install system packages. Sources are kept
outside version control under target/. SHA-256 checks detect unexpected changes.
"""
import argparse
import hashlib
from pathlib import Path
import tarfile
import urllib.request

THERMIMAGE = "cb244dac8fffaac50b1ba8bbbf58b7d37b5d96e1"
CSQ = "a0ea1a043a312e7869d23ce0df586ec480eef3e4"
EXIFTOOL = "1da714924f67763b6a8a7013dd2aa1983a16f87b"
SOURCES = {
    "raw2temp.R": (
        f"https://raw.githubusercontent.com/gtatters/Thermimage/{THERMIMAGE}/R/raw2temp.R",
        "2c1744363d8f36318d06955f1004dd98329e5277a9a6fd23d725e1b4850b8916"),
    "csq.py": (
        f"https://raw.githubusercontent.com/AlexanderProd/csq/{CSQ}/csq.py",
        "b95a12def836c05714b46789f3865dfb78bf6ec45b938f1cddc25534367d6954"),
    "exiftool.tar.gz": (
        f"https://codeload.github.com/exiftool/exiftool/tar.gz/{EXIFTOOL}",
        "9dde7c8027df6fe9a563febd9579fb821b42a01163129372885eaaeaf357c0d6"),
    "IR_2412.jpg": (
        f"https://raw.githubusercontent.com/gtatters/Thermimage/{THERMIMAGE}/inst/extdata/IR_2412.jpg",
        "2bd7ac42d752fcf6053d8fa54ef9315dfa8eab2f5b2c72a449f9c1a9af1c3a73"),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path,
                        default=Path("target/temperature-reference-tools"))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for name, (url, expected) in SOURCES.items():
        path = args.output / name
        if not path.exists():
            urllib.request.urlretrieve(url, path)
        if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise ValueError(f"SHA-256 mismatch: {path}")
        print("Verified", name)
    with tarfile.open(args.output / "exiftool.tar.gz") as archive:
        archive.extractall(args.output, filter="data")
    root = (args.output / f"exiftool-{EXIFTOOL}").resolve()
    (args.output / "exiftool-root.txt").write_text(str(root))
    print("ExifTool:", root / "exiftool")


if __name__ == "__main__":
    main()
