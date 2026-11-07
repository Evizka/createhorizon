#!/usr/bin/env python3
"""Fetch and unpack the offline Rust toolchain used to check this repository.

The sandbox has no system `rustc`/`cargo` and no access to `static.rust-lang.org`,
so the toolchain comes from the PyPI bundle `arena-rust-toolchain` (a split,
zstd-compressed Rust 1.97.0 distribution). The script is idempotent: if the
toolchain is already unpacked at the target directory it prints the environment
and exits.

Usage:
    python3 tools/bootstrap-rust.py [--target-dir DIR] [--print-env]
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tarfile

PACKAGE = "arena-rust-toolchain[all]==1.97.0"
VERSION = "1.97.0"
DATA_PACKAGES = (
    "arena_rust_toolchain_data1",
    "arena_rust_toolchain_data2",
    "arena_rust_toolchain_data3",
)
DEFAULT_TARGET = Path("~/.cache/rust").expanduser()


def log(message: str) -> None:
    print(f"[bootstrap-rust] {message}", file=sys.stderr)


def pip(*arguments: str) -> None:
    subprocess.run(
        [sys.executable, "-m", "pip", "install", "--no-cache-dir", "--break-system-packages", *arguments],
        check=True,
        text=True,
    )


def ensure_packages() -> None:
    try:
        import importlib

        for name in ("arena_rust_toolchain", *DATA_PACKAGES, "zstandard"):
            importlib.import_module(name)
    except ImportError:
        log("installing the bundled toolchain from PyPI")
        pip(PACKAGE, "zstandard")
    else:
        log("toolchain packages are already installed")


def part_paths() -> list[Path]:
    import importlib

    paths: list[Path] = []
    for name in DATA_PACKAGES:
        module = importlib.import_module(name)
        directory = Path(module.__path__[0])
        parts = sorted(
            path for path in directory.iterdir() if path.is_file() and not path.name.startswith("_")
        )
        if not parts:
            raise SystemExit(f"{name} contains no data files in {directory}")
        paths.extend(parts)
    return paths


def safe_extract(archive: tarfile.TarFile, destination: Path) -> None:
    """Extract a tar stream, rejecting absolute paths, `..` and symlinks."""
    destination.mkdir(parents=True, exist_ok=True)
    for member in archive:
        relative = PurePosixPath(member.name)
        if relative.is_absolute() or any(part in ("", ".", "..") for part in relative.parts):
            raise SystemExit(f"unsafe path in toolchain archive: {member.name}")
        if member.issym() or member.islnk():
            raise SystemExit(f"refusing link in toolchain archive: {member.name}")
        if not (member.isfile() or member.isdir()):
            raise SystemExit(f"unsupported entry in toolchain archive: {member.name}")
        archive.extract(member, destination)


def unpack(target: Path) -> Path:
    import zstandard

    combined = target / "_toolchain.tar.zst"
    target.mkdir(parents=True, exist_ok=True)
    log(f"combining {len(DATA_PACKAGES)} parts into {combined}")
    with combined.open("wb") as out:
        for part in part_paths():
            with part.open("rb") as source:
                shutil.copyfileobj(source, out, length=1024 * 1024)

    log(f"decompressing into {target}")
    with combined.open("rb") as compressed, zstandard.ZstdDecompressor().stream_reader(
        compressed
    ) as reader, tarfile.open(fileobj=reader, mode="r|") as archive:
        safe_extract(archive, target)

    combined.unlink()
    root = target / "prefix"
    if not root.is_dir():
        raise SystemExit(f"unexpected toolchain layout under {target}")
    return root


def toolchain_root(target: Path) -> Path:
    import importlib

    ensure_packages()
    root = target / "prefix"
    if not (root / "bin" / "cargo").is_file():
        root = unpack(target)
    return root


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, default=DEFAULT_TARGET)
    parser.add_argument("--print-env", action="store_true", help="print shell exports and exit")
    arguments = parser.parse_args()

    target = arguments.target_dir.expanduser().resolve()
    root = toolchain_root(target)
    bin_dir = root / "bin"
    for name in ("cargo", "rustc", "rustfmt", "clippy-driver", "cargo-clippy", "cargo-fmt"):
        if not (bin_dir / name).is_file():
            raise SystemExit(f"missing toolchain binary: {bin_dir / name}")

    version = subprocess.run(
        [str(bin_dir / "rustc"), "--version"], check=True, text=True, capture_output=True
    ).stdout.strip()
    log(version)
    print(bin_dir)
    if arguments.print_env:
        print(f'export PATH="{bin_dir}:$PATH"')
        print(f'export CARGO_HOME="{target / "cargo-home"}"')
        print(f'export RUSTUP_HOME="{target / "rustup-home"}"')
        os.makedirs(target / "cargo-home", exist_ok=True)
        os.makedirs(target / "rustup-home", exist_ok=True)


if __name__ == "__main__":
    main()
