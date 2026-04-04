#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import shutil
import sys
import tarfile
import zipfile
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="PauseInk release artifact packager"
    )
    parser.add_argument("--binary", required=True, help="Path to built binary")
    parser.add_argument("--platform", required=True, help="Artifact platform suffix")
    parser.add_argument("--version", required=True, help="Version or tag name")
    parser.add_argument(
        "--format",
        required=True,
        choices=("zip", "tar.gz"),
        help="Archive format to emit",
    )
    parser.add_argument(
        "--output-dir",
        required=True,
        help="Directory where the packaged artifact will be written",
    )
    return parser.parse_args()


def stage_payload(binary_path: Path, payload_root: Path) -> Path:
    if payload_root.exists():
        shutil.rmtree(payload_root)
    payload_root.mkdir(parents=True, exist_ok=True)
    staged_binary = payload_root / binary_path.name
    shutil.copy2(binary_path, staged_binary)

    readme_path = Path("README.md")
    if readme_path.is_file():
        shutil.copy2(readme_path, payload_root / readme_path.name)

    return payload_root


def sanitize_label(label: str) -> str:
    sanitized = re.sub(r"[^A-Za-z0-9._-]+", "-", label).strip("-")
    return sanitized or "artifact"


def create_zip(archive_path: Path, payload_root: Path) -> None:
    with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for source in payload_root.rglob("*"):
            if source.is_file():
                archive.write(source, source.relative_to(payload_root.parent))


def create_tar_gz(archive_path: Path, payload_root: Path) -> None:
    with tarfile.open(archive_path, "w:gz") as archive:
        archive.add(payload_root, arcname=payload_root.name)


def main() -> int:
    args = parse_args()
    binary_path = Path(args.binary)
    if not binary_path.is_file():
        print(f"binary not found: {binary_path}", file=sys.stderr)
        return 1

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    archive_stem = f"pauseink-{sanitize_label(args.version)}-{args.platform}"
    payload_root = output_dir / archive_stem
    stage_payload(binary_path, payload_root)

    try:
        if args.format == "zip":
            archive_path = output_dir / f"{archive_stem}.zip"
            create_zip(archive_path, payload_root)
        else:
            archive_path = output_dir / f"{archive_stem}.tar.gz"
            create_tar_gz(archive_path, payload_root)
    finally:
        if payload_root.exists():
            shutil.rmtree(payload_root)

    print(archive_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
