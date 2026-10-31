#!/usr/bin/env python3
"""Stage and build the pinned native Create Horizon mod against Horizon source."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import posixpath
import shutil
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
COMPAT = json.loads((ROOT / "compatibility.json").read_text(encoding="utf-8"))
PIN = COMPAT["horizon_revision"]
MIXINS = Path(os.environ.get("HORIZON_MIXINS", ROOT.parent / "horizon-mixins")).resolve()


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def run(command: list[str], *, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, cwd=cwd, check=True, text=True, capture_output=True)


def safe_extract(archive: tarfile.TarFile, destination: Path) -> None:
    for member in archive:
        relative = PurePosixPath(member.name)
        if relative.is_absolute() or any(part in ("", ".", "..") for part in relative.parts):
            raise ValueError(f"unsafe path in pinned source archive: {member.name}")
        target = destination.joinpath(*relative.parts)
        if member.isdir():
            target.mkdir(parents=True, exist_ok=True)
        elif member.isfile():
            target.parent.mkdir(parents=True, exist_ok=True)
            source = archive.extractfile(member)
            if source is None:
                raise ValueError(f"cannot read pinned source file: {member.name}")
            with source, target.open("wb") as output:
                shutil.copyfileobj(source, output)
        elif member.issym():
            link = PurePosixPath(member.linkname)
            resolved = posixpath.normpath((relative.parent / link).as_posix())
            if link.is_absolute() or resolved == ".." or resolved.startswith("../"):
                raise ValueError(f"unsafe symlink in pinned source archive: {member.name}")
            target.parent.mkdir(parents=True, exist_ok=True)
            target.symlink_to(member.linkname)
        else:
            raise ValueError(f"unsupported entry in pinned source archive: {member.name}")


def export_horizon(horizon: Path, stages: Path) -> Path:
    run(["git", "-C", str(horizon), "cat-file", "-e", PIN + "^{commit}"])
    inputs = {"revision": PIN.encode(), "export-schema": b"1"}
    for relative, expected in sorted(COMPAT["source_sha256"].items()):
        inputs[relative] = expected.encode()
    signature = digest(b"".join(name.encode() + b"\0" + value for name, value in sorted(inputs.items())))
    stages.mkdir(parents=True, exist_ok=True)
    output = stages / "original" / signature[:20]
    marker_path = output / ".createhorizon-export.json"
    if output.exists():
        marker = json.loads(marker_path.read_text(encoding="utf-8"))
        if marker.get("signature") != signature:
            raise ValueError(f"pinned export marker mismatch: {output}")
        for relative, expected in marker["files"].items():
            if digest((output / relative).read_bytes()) != expected:
                raise ValueError(f"pinned export was edited: {relative}; remove {output} to recreate it")
        return output

    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=".createhorizon-export-", dir=output.parent))
    process = subprocess.Popen(
        ["git", "-C", str(horizon), "archive", "--format=tar", PIN],
        stdout=subprocess.PIPE,
    )
    try:
        assert process.stdout is not None
        with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
            safe_extract(archive, temporary)
        process.stdout.close()
        code = process.wait()
        if code:
            raise ValueError(f"git archive exited with status {code}")
        for relative, expected in COMPAT["source_sha256"].items():
            path = temporary / relative
            if digest(path.read_bytes()) != expected:
                raise ValueError(f"pinned source hash mismatch: {relative}")
        # The mixin source snapshot rejects symlinks; materialize only links
        # from this trusted, commit-pinned archive.
        for path in sorted(temporary.rglob("*")):
            if path.is_symlink():
                payload = path.read_bytes()
                path.unlink()
                path.write_bytes(payload)
        files = {
            str(path.relative_to(temporary)): digest(path.read_bytes())
            for path in sorted(temporary.rglob("*"))
            if path.is_file()
        }
        (temporary / ".horizon-revision").write_text(PIN + "\n", encoding="utf-8")
        (temporary / ".createhorizon-export.json").write_text(
            json.dumps({"signature": signature, "files": files}, sort_keys=True, indent=2) + "\n",
            encoding="utf-8",
        )
        os.rename(temporary, output)
    except BaseException:
        if process.stdout is not None:
            process.stdout.close()
        if process.poll() is None:
            process.kill()
            process.wait()
        shutil.rmtree(temporary, ignore_errors=True)
        raise
    return output


def build_stage(horizon: Path, stages: Path) -> tuple[Path, Path]:
    revision = run(["git", "-C", str(MIXINS), "rev-parse", "HEAD"]).stdout.strip()
    if revision != COMPAT["horizon_mixins_revision"]:
        raise ValueError(
            f"horizon-mixins must be at {COMPAT['horizon_mixins_revision']}, got {revision}"
        )
    run(["git", "-C", str(MIXINS), "diff", "--quiet", "HEAD", "--", "Cargo.toml", "Cargo.lock", "crates"])
    original = export_horizon(horizon, stages)
    cli_target = ROOT / "target" / "horizon-mixins-cargo"
    subprocess.run(
        [
            "cargo", "build", "--locked", "--offline", "--target-dir", str(cli_target),
            "-p", "horizon-mixins-cli",
        ],
        cwd=MIXINS,
        check=True,
    )
    cli = cli_target / "debug" / "horizon-mixins"
    output = stages / "modded"
    plan = stages / "plan.json"
    base = ["--workspace", str(original), "--mods", str(ROOT / "mixins")]
    run([str(cli), "plan", *base, "--out", str(plan)])
    applied = run([str(cli), "apply", *base, "--out", str(output)])
    report = json.loads(applied.stdout)
    return output, plan


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("stage", "check", "test", "build"))
    parser.add_argument("--horizon", type=Path, default=ROOT.parent / "horizon")
    parser.add_argument("--target-dir", type=Path, default=ROOT / "target" / "horizon-cargo")
    args = parser.parse_args()
    horizon = args.horizon.resolve()
    target = args.target_dir.resolve()
    if target.is_relative_to(horizon):
        parser.error("Cargo target must stay outside the original Horizon checkout")
    stages = ROOT / "target" / "mixins"
    modded, plan = build_stage(horizon, stages)
    if target.is_relative_to(modded) or modded.is_relative_to(target):
        parser.error("Cargo target and generated source stage must be separate")
    report: dict[str, object] = {
        "stage": str(modded),
        "plan": str(plan),
        "horizon_revision": PIN,
        "horizon_mixins_revision": COMPAT["horizon_mixins_revision"],
    }
    if args.action != "stage":
        command = [
            "cargo", args.action, "--locked", "--offline", "-p", "horizon-client",
            "--bin", "horizon-client", "--no-default-features", "--target-dir", str(target),
        ]
        if args.action == "build":
            command.extend(("--profile", "fast"))
        if args.action == "test":
            command.extend(("game::createhorizon_native::tests", "--", "--nocapture"))
        result = subprocess.run(command, cwd=modded, capture_output=True, text=True)
        log = ROOT / "target" / f"horizon-{args.action}.log"
        log.write_text(result.stdout + result.stderr, encoding="utf-8")
        report.update({"cargo_exit": result.returncode, "cargo_log": str(log)})
        if result.returncode:
            print((result.stdout + result.stderr)[-16000:], file=sys.stderr)
            print(json.dumps(report, indent=2), file=sys.stderr)
            return result.returncode
        if args.action == "build":
            report["binary"] = str(target / "fast" / "horizon-client")
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        if isinstance(error, subprocess.CalledProcessError) and error.stderr:
            print(error.stderr, file=sys.stderr)
        print(f"Create Horizon native build: {error}", file=sys.stderr)
        raise SystemExit(1)
