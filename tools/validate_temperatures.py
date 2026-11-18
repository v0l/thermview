#!/usr/bin/env python3
"""Compare ThermView with ExifTool + FFmpeg/Pillow + two upstream converters.

The reference decoding and metadata extraction never call ThermView. The CSQ
reference uses only upstream raw2temp, with explicit percent humidity inputs.
Full rasters remain under the ignored target directory, not in Git.
"""
import argparse
import ast
import csv
import hashlib
import io
import json
import math
from pathlib import Path
import struct
import subprocess
import sys

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
FIELDS = {
    "E": "Emissivity", "OD": "ObjectDistance",
    "RTemp": "ReflectedApparentTemperature", "ATemp": "AtmosphericTemperature",
    "IRWTemp": "IRWindowTemperature", "IRT": "IRWindowTransmission",
    "RH": "RelativeHumidity", "PR1": "PlanckR1", "PB": "PlanckB",
    "PF": "PlanckF", "PO": "PlanckO", "PR2": "PlanckR2",
    "ATA1": "AtmosphericTransAlpha1", "ATA2": "AtmosphericTransAlpha2",
    "ATB1": "AtmosphericTransBeta1", "ATB2": "AtmosphericTransBeta2",
    "ATX": "AtmosphericTransX",
}
STANDARD_ATMOSPHERE = dict(ATA1=0.006569, ATA2=0.01262, ATB1=-0.002276,
                           ATB2=-0.00667, ATX=1.9)


def run(command, **kwargs):
    return subprocess.check_output([str(v) for v in command], **kwargs)


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def index_csq(path):
    """Independent streaming directory walker; no ThermView code/imports."""
    frames = []
    with path.open("rb") as stream:
        size = stream.seek(0, 2)
        start = 0
        while start < size:
            stream.seek(start)
            header = stream.read(64)
            if header[:4] != b"FFF\0":
                raise ValueError(f"Expected FFF header at {start}")
            order = "<" if 100 <= struct.unpack_from("<I", header, 20)[0] < 200 else ">"
            version, index, count = struct.unpack_from(order + "III", header, 20)
            if not 100 <= version < 200 or not 1 <= count <= 100:
                raise ValueError("Unsupported FFF directory")
            stream.seek(start + index)
            directory = stream.read(count * 32)
            end = index + count * 32
            for at in range(0, count * 32, 32):
                entry = struct.unpack_from(order + "HH7I", directory, at)
                if entry[0]:
                    end = max(end, entry[4] + entry[5])
            if start + end > size:
                raise ValueError("Truncated FFF frame")
            frames.append((start, end))
            start += end
    return frames


def reference_raw(exiftool, source, folder, meta):
    encoded = run(["perl", exiftool, "-b", "-RawThermalImage", source])
    width, height = int(meta["RawThermalImageWidth"]), int(meta["RawThermalImageHeight"])
    if encoded.startswith(b"\xff\xd8"):
        encoded_path = folder / "reference.jls"
        encoded_path.write_bytes(encoded)
        result = subprocess.run(
            ["ffmpeg", "-v", "error", "-err_detect", "explode", "-i", str(encoded_path),
             "-f", "rawvideo", "-pix_fmt", "gray16le", "pipe:1"], capture_output=True,
        )
        if result.returncode:
            raise ValueError(result.stderr.decode(errors="replace").strip())
        raw = np.frombuffer(result.stdout, dtype="<u2")
    else:
        raw = np.asarray(Image.open(io.BytesIO(encoded)))
        if encoded.startswith(b"\x89PNG"):
            # FLIR embeds little-endian samples inside the big-endian PNG format.
            raw = raw.astype(np.uint16).byteswap()
    if raw.size != width * height:
        raise ValueError(f"Wrong reference raster size: {raw.size} versus {width * height}")
    return raw.astype(np.uint16).reshape(height, width)


def parameters(meta):
    result = {key: float(meta[tag]) for key, tag in FIELDS.items()}
    # ExifTool -n returns RelativeHumidity as a fraction; both converters want %.
    result["RH"] *= 100.0
    return result


def thermimage(tools, raw, params, folder):
    np.savetxt(folder / "reference-counts.txt", raw.ravel(), fmt="%d")
    with (folder / "parameters.csv").open("w") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(params))
        writer.writeheader()
        writer.writerow(params)
    run(["Rscript", ROOT / "tools/temperature_reference.R", tools / "raw2temp.R",
         folder / "reference-counts.txt", folder / "parameters.csv",
         folder / "thermimage.txt"], stderr=subprocess.STDOUT)
    return np.genfromtxt(folder / "thermimage.txt", missing_values="NA",
                         filling_values=np.nan).reshape(raw.shape)


def load_csq_converter(path):
    # Execute the unmodified upstream function, not its optional UI/decoder imports.
    tree = ast.parse(path.read_text())
    node = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "raw2temp")
    namespace = {"exp": np.exp, "sqrt": np.sqrt, "log": np.log}
    exec(compile(ast.Module(body=[node], type_ignores=[]), str(path), "exec"), namespace)
    return namespace["raw2temp"]


def compare(a, b):
    valid_a = np.isfinite(a) & (a > -273.1)
    valid_b = np.isfinite(b) & (b > -273.1)
    valid = valid_a & valid_b
    difference = np.abs(a[valid] - b[valid])
    return dict(pixels=int(a.size), compared=int(valid.sum()),
                validity_mismatches=int(np.count_nonzero(valid_a != valid_b)),
                max_abs_c=float(difference.max()) if difference.size else None,
                mean_abs_c=float(difference.mean()) if difference.size else None)


def stats(values):
    valid = np.isfinite(values) & (values > -273.1)
    return {"valid_pixels": int(valid.sum()), "invalid_pixels": int((~valid).sum()),
            "min_c": float(values[valid].min()), "max_c": float(values[valid].max()),
            "mean_c": float(values[valid].mean())}


def preview_independence(args):
    """Replace the visible JPEG with two flat colours, preserving FLIR APP1."""
    if not args.image:
        return None
    source = args.image[0]
    jpeg = source.read_bytes()
    if not jpeg.startswith(b"\xff\xd8"):
        return None
    segments = []
    at = 2
    while at + 4 <= len(jpeg) and jpeg[at] == 0xff:
        marker = jpeg[at + 1]
        if marker in (0xda, 0xd9):
            break
        length = int.from_bytes(jpeg[at + 2:at + 4], 'big')
        if marker == 0xe1 and jpeg[at + 4:at + 9] == b"FLIR\0":
            segments.append(jpeg[at:at + 2 + length])
        at += 2 + length
    if not segments:
        return None
    baseline = args.output / "image-01"
    checks = []
    for name, colour in [("black", (0, 0, 0)), ("magenta", (255, 0, 255))]:
        folder = args.output / f"preview-{name}"
        folder.mkdir(exist_ok=True)
        visible = io.BytesIO()
        Image.new("RGB", (320, 240), colour).save(visible, format="JPEG")
        altered = folder / "altered.jpg"
        altered.write_bytes(b"\xff\xd8" + b"".join(segments) + visible.getvalue()[2:])
        run([args.dump, altered, folder])
        checks.append({"colour": name, "visible_jpeg_changed": sha256(altered) != sha256(source),
                       "raw_counts_identical": (folder / "sensor.u16").read_bytes()
                       == (baseline / "sensor.u16").read_bytes(),
                       "temperatures_identical": (folder / "temperature.f32").read_bytes()
                       == (baseline / "temperature.f32").read_bytes()})
    return checks


def check_case(args, converter, source, folder, label):
    record = {"case": label, "source_sha256": sha256(source)}
    meta = json.loads(run(["perl", args.exiftool, "-j", "-n", source]))[0]
    (folder / "exiftool.json").write_text(json.dumps(meta, indent=2))
    record["source_metadata"] = {tag: meta.get(tag) for tag in FIELDS.values()}
    try:
        raw = reference_raw(args.exiftool, source, folder, meta)
        params = parameters(meta)
        reference = thermimage(args.tools, raw, params, folder)
        csq_meta = {"FLIR:" + tag: params[key] for key, tag in FIELDS.items()}
        other = converter(raw.astype(np.float64), csq_meta)
        record["reference_decoded"] = True
        record["thermimage"] = stats(reference)
        record["csq_converter"] = stats(other)
        record["reference_agreement"] = compare(reference, other)
    except Exception as error:
        record["reference_decoded"] = False
        record["reference_error"] = str(error)
    if (folder / "error.txt").exists():
        record["thermview_error"] = (folder / "error.txt").read_text()
        record["status"] = "THERMVIEW_DECODE_FAILED"
        return record
    tv = json.loads((folder / "metadata.json").read_text())
    temperatures = np.fromfile(folder / "temperature.f32", dtype="<f4").astype(float)
    temperatures = temperatures.reshape(tv["height"], tv["width"])
    record["thermview"] = stats(temperatures)
    record["thermview_metadata"] = tv
    if not record["reference_decoded"]:
        record["status"] = "REFERENCE_FAILED"
        return record
    counts = np.fromfile(folder / "sensor.u16", dtype="<u2").reshape(raw.shape)
    record["raw_count_mismatches"] = int(np.count_nonzero(counts != raw))
    record["metadata_differences"] = {}
    for key, field in {"emissivity": "E", "distance": "OD", "reflected": "RTemp",
                       "air": "ATemp", "humidity": "RH"}.items():
        if not math.isclose(tv[key], params[field], rel_tol=1e-12, abs_tol=1e-9):
            record["metadata_differences"][key] = {"file": params[field], "thermview": tv[key]}
    record["planck_constants_match"] = all(
        math.isclose(tv["planck"][key], params[field], rel_tol=1e-12, abs_tol=1e-9)
        for key, field in {"r1": "PR1", "b": "PB", "f": "PF", "o": "PO", "r2": "PR2"}.items()
    )
    record["temperature_agreement"] = compare(temperatures, reference)
    # Diagnostic only: explain conversion differences with ThermView's assumptions.
    # This is NOT substituted for the file-metadata comparison above.
    aligned = dict(params, **STANDARD_ATMOSPHERE)
    aligned.update(E=tv["emissivity"], OD=tv["distance"], RTemp=tv["reflected"],
                   ATemp=tv["air"], RH=tv["humidity"], IRT=1, IRWTemp=tv["air"])
    # Newer dumps retain the file-specific atmosphere and IR-window settings.
    aligned.update(tv.get("environment") or {})
    other_aligned = converter(raw.astype(float),
                             {"FLIR:" + tag: aligned[key] for key, tag in FIELDS.items()})
    record["thermview_assumptions_diagnostic"] = compare(temperatures, other_aligned)
    agreement = record["temperature_agreement"]
    record["status"] = "PASS" if (
        record["raw_count_mismatches"] == 0 and agreement["validity_mismatches"] == 0
        and agreement["compared"] == agreement["pixels"]
        and agreement["max_abs_c"] <= args.tolerance
        and record["reference_agreement"]["max_abs_c"] <= 1e-8
    ) else "DIFFERENCE"
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--csq", type=Path)
    parser.add_argument("--image", type=Path, action="append", default=[])
    parser.add_argument("--frames", help="One-based comma-separated CSQ frames")
    parser.add_argument("--tools", type=Path, required=True)
    parser.add_argument("--exiftool", type=Path, required=True)
    parser.add_argument("--dump", type=Path, default=ROOT / "target/debug/examples/temperature_dump")
    parser.add_argument("--output", type=Path, default=ROOT / "target/temperature-validation")
    parser.add_argument("--tolerance", type=float, default=0.001)
    args = parser.parse_args()
    if not args.csq and not args.image:
        parser.error("provide at least one --csq or --image")
    args.output.mkdir(parents=True, exist_ok=True)
    converter = load_csq_converter(args.tools / "csq.py")
    manifest = {
        "thermview_commit": run(["git", "-C", ROOT, "rev-parse", "HEAD"], text=True).strip(),
        "tolerance_c": args.tolerance,
        "exiftool_version": run(["perl", args.exiftool, "-ver"], text=True).strip(),
        "ffmpeg_version": run(["ffmpeg", "-version"], text=True).splitlines()[0],
        "r_version": run(["Rscript", "--version"], stderr=subprocess.STDOUT, text=True).strip(),
        "reference_hashes": {name: sha256(args.tools / name) for name in ["raw2temp.R", "csq.py"]},
        "cases": [],
    }
    cases = []
    if args.csq:
        frames = index_csq(args.csq)
        manifest["csq"] = {"name": args.csq.name, "sha256": sha256(args.csq),
                           "total_frames": len(frames)}
        chosen = sorted(set(map(int, args.frames.split(',')))) if args.frames else sorted(set(
            np.linspace(1, len(frames), 12).astype(int).tolist()
            + [n for n in [1, 2, 3, 5526, 5527, 5528, 11147, 11148, 11149] if n <= len(frames)]))
        if any(n < 1 or n > len(frames) for n in chosen):
            parser.error(f"frame numbers must be between 1 and {len(frames)}")
        for frame in chosen:
            (args.output / "csq" / f"frame-{frame:05}" / "error.txt").unlink(missing_ok=True)
        run([args.dump, args.csq, args.output / "csq", ','.join(map(str, chosen))])
        with args.csq.open("rb") as stream:
            for frame in chosen:
                start, length = frames[frame - 1]
                stream.seek(start)
                folder = args.output / "csq" / f"frame-{frame:05}"
                source = folder / "source.fff"
                source.write_bytes(stream.read(length))
                cases.append((source, folder, f"CSQ frame {frame}"))
    for i, source in enumerate(args.image):
        folder = args.output / f"image-{i + 1:02}"
        folder.mkdir(parents=True, exist_ok=True)
        (folder / "error.txt").unlink(missing_ok=True)
        result = subprocess.run([str(args.dump), str(source), str(folder)], capture_output=True)
        if result.returncode:
            (folder / "error.txt").write_bytes(result.stderr)
        cases.append((source, folder, source.name))
    for source, folder, label in cases:
        record = check_case(args, converter, source, folder, label)
        manifest["cases"].append(record)
        (args.output / "results.json").write_text(json.dumps(manifest, indent=2, allow_nan=False))
        print(label, record["status"], record.get("temperature_agreement", {}), flush=True)
    manifest["preview_independence"] = preview_independence(args)
    (args.output / "results.json").write_text(json.dumps(manifest, indent=2, allow_nan=False))
    print("Results:", args.output / "results.json")
    preview_failed = any(not all(c[k] for k in (
        "visible_jpeg_changed", "raw_counts_identical", "temperatures_identical"
    )) for c in manifest["preview_independence"] or [])
    return int(preview_failed or any(c["status"] != "PASS" for c in manifest["cases"]))


if __name__ == "__main__":
    sys.exit(main())
