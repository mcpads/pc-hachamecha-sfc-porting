import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent


def run(args, **kwargs):
    subprocess.run(list(map(str, args)), cwd=ROOT, check=True, **kwargs)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def strip_module(data):
    if data[:8] != b"\0asm\x01\0\0\0":
        raise ValueError("Invalid WebAssembly module")
    out = bytearray(data[:8])
    at = 8
    while at < len(data):
        start = at
        kind = data[at]
        at += 1
        length = shift = 0
        while True:
            byte = data[at]
            at += 1
            length |= (byte & 127) << shift
            if byte < 128:
                break
            shift += 7
        end = at + length
        if end > len(data):
            raise ValueError("Truncated WebAssembly module")
        if kind:
            out.extend(data[start:end])
        at = end
    return bytes(out)


def build(release):
    work = ROOT / ".build"
    work.mkdir(exist_ok=True)
    flags = f"--remap-path-prefix={Path.home()}=/build --remap-path-prefix={ROOT}=/converter"
    run(
        [
            "npm",
            "ci",
            "--prefix",
            ROOT / "web",
            "--ignore-scripts",
            "--no-audit",
            "--no-fund",
        ]
    )
    for name, manifest in [
        ("game", "port-build/Cargo.toml"),
        ("audio", "tad/Cargo.toml"),
    ]:
        args = [
            "cargo",
            "build",
            "--release",
            "--locked",
            "--target",
            "wasm32-wasip1",
            "--manifest-path",
            ROOT / manifest,
        ]
        if name == "audio":
            args += ["-p", "tad-compiler"]
        run(
            args,
            env=dict(os.environ, CARGO_TARGET_DIR=str(work / name), RUSTFLAGS=flags),
        )
    contract = json.loads((ROOT / "assets/audio.json").read_text())
    with tempfile.TemporaryDirectory(dir=work, prefix="site-") as temp:
        out = Path(temp) / "site"
        out.mkdir()
        for target, source in [
            (
                "builder.wasm",
                work / "game/wasm32-wasip1/release/hachamecha-port-build.wasm",
            ),
            ("tad.wasm", work / "audio/wasm32-wasip1/release/tad-compiler.wasm"),
        ]:
            (out / target).write_bytes(strip_module(source.read_bytes()))
        for name, expected in contract["modules"].items():
            matches = [
                p
                for p in (work / "audio/wasm32-wasip1/release/build").glob(
                    f"compiler-*/out/{name}"
                )
                if sha(p.read_bytes()) == expected
            ]
            if not matches:
                raise ValueError(f"Audio driver identity mismatch: {name}")
            shutil.copyfile(matches[0], out / name)
        fonts = {
            "neodgm.ttf": "d61b60eccb731f8ca9c7da582e4a05a94db66b570471809950aa9a7261b941d6",
            "Galmuri11.ttf": "2c709890595668f7bdb6df408420fda957dde0288e95b31a1cc17a2ab98b4b4f",
        }
        for name, expected in fonts.items():
            data = (ROOT / "fonts" / name).read_bytes()
            if sha(data) != expected:
                raise ValueError(f"Font identity mismatch: {name}")
            (out / name).write_bytes(data)
        for source, target in [
            ("audio.json", "audio.json"),
            ("music.json", "music.json"),
        ]:
            shutil.copyfile(ROOT / "assets" / source, out / target)
        shutil.copytree(ROOT / "licenses", out / "licenses")
        esbuild = ROOT / "web/node_modules/.bin/esbuild"
        for name in ("app.mjs", "style.css"):
            run(
                [
                    esbuild,
                    ROOT / "web" / name,
                    "--minify",
                    "--legal-comments=external",
                    f"--outfile={out / name}",
                ]
            )
        run(
            [
                esbuild,
                ROOT / "web/runtime/worker.mjs",
                "--bundle",
                "--format=esm",
                "--platform=browser",
                "--minify",
                "--legal-comments=external",
                f"--outfile={out}/worker.mjs",
            ]
        )
        for path in out.rglob("*.LEGAL.txt"):
            if path.stat().st_size:
                path.rename(out / "licenses" / path.name)
            else:
                path.unlink()
        shutil.copyfile(ROOT / "web/index.html", out / "index.html")
        inventory = {
            str(p.relative_to(out)): {
                "bytes": p.stat().st_size,
                "sha256": sha(p.read_bytes()),
            }
            for p in sorted(out.rglob("*"))
            if p.is_file()
        }
        (out / "manifest.json").write_text(
            json.dumps(
                {
                    "schema": 1,
                    "build": {
                        "input_contract": "KR DiskStation issue 1",
                        "game_assets_included": False,
                    },
                    "files": inventory,
                },
                separators=(",", ":"),
            )
        )
        destination = ROOT / "dist"
        if destination.exists():
            if not (destination / "manifest.json").is_file():
                raise ValueError("Unrecognized dist directory")
            previous = Path(temp) / "previous"
            destination.rename(previous)
            try:
                out.rename(destination)
            except OSError:
                previous.rename(destination)
                raise
        else:
            out.rename(destination)
    if release:
        directory = ROOT / "release"
        directory.mkdir(exist_ok=True)
        archive = directory / "madou-sfc-patcher.zip"
        with zipfile.ZipFile(
            archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
        ) as target:
            for path in sorted(destination.rglob("*")):
                if path.is_file():
                    info = zipfile.ZipInfo(
                        path.relative_to(destination).as_posix(), (1980, 1, 1, 0, 0, 0)
                    )
                    info.compress_type = zipfile.ZIP_DEFLATED
                    info.external_attr = 0o100644 << 16
                    target.writestr(info, path.read_bytes())
        (directory / "SHA256SUMS").write_text(
            f"{sha(archive.read_bytes())}  {archive.name}\n"
        )
        print(archive)
    print(destination)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--release", action="store_true")
    build(parser.parse_args().release)
