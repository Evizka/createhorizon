#!/usr/bin/env python3
"""Validate the source bundles and make reproducible Horizon .hmod/.hnmod files."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path, PurePosixPath
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
CONTENT = ROOT / "content" / "createhorizon"
BUNDLE = ROOT / "mixins" / "createhorizon_native"
OUTPUT = ROOT / "artifacts"
EXPECTED_BLOCKS = {
    "hand_crank",
    "shaft",
    "vertical_shaft",
    "small_cogwheel",
    "large_cogwheel",
    "millstone",
    "crushing_wheel",
    "water_wheel",
    "gearbox",
    "speedometer",
    "clutch",
}


def read_json(path: Path) -> object:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"{path.relative_to(ROOT)}: invalid JSON: {error}") from error


def source_files(root: Path, allowed_suffixes: set[str]) -> dict[str, bytes]:
    files: dict[str, bytes] = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise SystemExit(f"refusing symlink: {path.relative_to(ROOT)}")
        if not path.is_file():
            continue
        relative = path.relative_to(root).as_posix()
        if path.suffix not in allowed_suffixes:
            raise SystemExit(f"unexpected source file: {path.relative_to(ROOT)}")
        pure = PurePosixPath(relative)
        if pure.is_absolute() or ".." in pure.parts or "\\" in relative:
            raise SystemExit(f"invalid bundle path: {relative}")
        files[relative] = path.read_bytes()
    return files


def validate_content() -> tuple[dict[str, bytes], str, str]:
    manifest_path = CONTENT / "horizon-mod.toml"
    manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("id") != "createhorizon" or manifest.get("api") != "0.2":
        raise SystemExit("content manifest must be createhorizon API 0.2")
    if "registries" not in manifest.get("capabilities", []):
        raise SystemExit("content manifest must declare the registries capability")

    files = {"horizon-mod.toml": manifest_path.read_bytes()}
    for root_name in ("assets", "data"):
        root = CONTENT / root_name
        for path in sorted(root.rglob("*")):
            if path.is_symlink():
                raise SystemExit(f"refusing symlink: {path.relative_to(ROOT)}")
            if not path.is_file():
                continue
            relative = path.relative_to(CONTENT).as_posix()
            if path.suffix == ".json":
                read_json(path)
            files[relative] = path.read_bytes()

    blocks = {
        PurePosixPath(name).stem
        for name in files
        if name.startswith("data/createhorizon/horizon/block/") and name.endswith(".json")
    }
    if blocks != EXPECTED_BLOCKS:
        raise SystemExit(f"expected {len(EXPECTED_BLOCKS)} sample blocks, found {sorted(blocks)}")
    for block in sorted(blocks):
        model_path = f"assets/createhorizon/models/block/{block}.json"
        state_path = f"assets/createhorizon/blockstates/{block}.json"
        item_path = f"assets/createhorizon/items/{block}.json"
        loot_path = f"data/createhorizon/loot_table/blocks/{block}.json"
        recipe_path = f"data/createhorizon/recipe/{block}.json"
        for required in (model_path, state_path, item_path, loot_path, recipe_path):
            if required not in files:
                raise SystemExit(f"missing content file: {required}")
    return files, manifest["id"], manifest["version"]


def validate_native(compatibility: dict[str, object]) -> tuple[dict[str, bytes], str, str]:
    mixin_path = BUNDLE / "horizon-mixin.toml"
    mixin = tomllib.loads(mixin_path.read_text(encoding="utf-8"))
    mod = mixin.get("mod", {})
    compatibility_table = mixin.get("compatibility", {})
    if mod.get("id") != BUNDLE.name:
        raise SystemExit("native manifest id must match its mixins/<id>/ folder")
    if compatibility_table.get("horizon_revision") != compatibility.get("horizon_revision"):
        raise SystemExit("native manifest and compatibility.json pin different Horizon commits")
    if not mixin.get("crate") or mixin["crate"][0].get("path") != "engine":
        raise SystemExit("native bundle must declare the engine bundle crate")
    engine_manifest = tomllib.loads((BUNDLE / "engine" / "Cargo.toml").read_text(encoding="utf-8"))
    if engine_manifest.get("package", {}).get("name") != "create-engine":
        raise SystemExit("native engine package must be named create-engine")
    if "workspace" in engine_manifest or (BUNDLE / "engine" / "Cargo.lock").exists():
        raise SystemExit("Horizon bundle crates cannot declare a workspace or include Cargo.lock")

    metadata = {
        "schema": 1,
        "id": mod["id"],
        "version": mod["version"],
        "game": compatibility["game"],
        "horizon": compatibility["horizon_revision"],
        "mixins": compatibility["horizon_mixins_revision"],
        "hooks": compatibility["horizon_native_hooks_api"],
    }
    # A helper named in the manifest that the bundle does not carry would
    # only fail later, in the launcher's build.
    for injection in mixin.get("injection", []):
        helper = injection.get("helper")
        if not helper:
            raise SystemExit(f"injection {injection.get('id')!r} names no helper")
        if not (BUNDLE / helper).is_file():
            raise SystemExit(f"injection {injection.get('id')!r} names missing helper {helper}")
        if not injection.get("bindings"):
            raise SystemExit(f"injection {injection.get('id')!r} declares no bindings")
    for module in mixin.get("module", []):
        source = module.get("source")
        if not source or not (BUNDLE / source).is_file():
            raise SystemExit(f"module {module.get('id')!r} names missing source {source}")

    files = {
        "horizon-native.json": (json.dumps(metadata, indent=2, sort_keys=True) + "\n").encode()
    }
    for relative, payload in source_files(BUNDLE, {".rs", ".toml"}).items():
        files[f"mixins/{mod['id']}/{relative}"] = payload
    if f"mixins/{mod['id']}/horizon-mixin.toml" not in files:
        raise SystemExit("native .hnmod is missing horizon-mixin.toml")
    return files, mod["id"], mod["version"]


def write_zip(path: Path, files: dict[str, bytes]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_STORED) as archive:
        for name, content in sorted(files.items()):
            info = zipfile.ZipInfo(name, date_time=(2026, 10, 30, 0, 0, 0))
            info.compress_type = zipfile.ZIP_STORED
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            archive.writestr(info, content)


def describe(path: Path) -> dict[str, object]:
    payload = path.read_bytes()
    return {
        "file": str(path.relative_to(ROOT)),
        "bytes": len(payload),
        "sha256": hashlib.sha256(payload).hexdigest(),
    }


def main() -> None:
    compatibility = json.loads((ROOT / "compatibility.json").read_text(encoding="utf-8"))
    for key in ("horizon_revision", "horizon_mixins_revision"):
        value = compatibility.get(key)
        if not isinstance(value, str) or len(value) != 40:
            raise SystemExit(f"compatibility.json: {key} must be a full 40-character commit")

    content_files, content_id, content_version = validate_content()
    native_files, native_id, native_version = validate_native(compatibility)
    content_path = OUTPUT / f"{content_id}-{content_version}.hmod"
    native_path = OUTPUT / f"{native_id}-{native_version}.hnmod"
    write_zip(content_path, content_files)
    write_zip(native_path, native_files)
    print(json.dumps({"packages": [describe(content_path), describe(native_path)]}, indent=2))


if __name__ == "__main__":
    main()
