#!/usr/bin/env python3
from __future__ import annotations

import contextlib
import importlib.util
import os
import tempfile
import unittest
import zipfile
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("package_release_asset.py")
SPEC = importlib.util.spec_from_file_location("package_release_asset", MODULE_PATH)
package_release_asset = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(package_release_asset)


@contextlib.contextmanager
def chdir(path: Path):
    previous = Path.cwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(previous)


class PackageReleaseAssetTests(unittest.TestCase):
    def test_stage_payload_copies_presets_tree(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            binary_path = root / "pauseink.exe"
            binary_path.write_bytes(b"exe")
            (root / "README.md").write_text("readme", encoding="utf-8")
            (root / "presets/style_presets").mkdir(parents=True)
            (root / "presets/export_profiles").mkdir(parents=True)
            (root / "presets/style_presets/marker.json5").write_text(
                "{ id: 'marker' }",
                encoding="utf-8",
            )
            (root / "presets/export_profiles/medium.json5").write_text(
                "{ id: 'medium' }",
                encoding="utf-8",
            )

            payload_root = root / "dist/pauseink-v1.0.0-windows-x86_64"
            with chdir(root):
                package_release_asset.stage_payload(binary_path, payload_root)

            self.assertTrue((payload_root / "pauseink.exe").is_file())
            self.assertTrue((payload_root / "README.md").is_file())
            self.assertTrue((payload_root / "presets/style_presets/marker.json5").is_file())
            self.assertTrue((payload_root / "presets/export_profiles/medium.json5").is_file())

    def test_zip_archive_contains_presets_tree(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            binary_path = root / "pauseink.exe"
            binary_path.write_bytes(b"exe")
            (root / "README.md").write_text("readme", encoding="utf-8")
            (root / "presets/style_presets").mkdir(parents=True)
            (root / "presets/export_profiles").mkdir(parents=True)
            (root / "presets/style_presets/marker.json5").write_text(
                "{ id: 'marker' }",
                encoding="utf-8",
            )
            (root / "presets/export_profiles/medium.json5").write_text(
                "{ id: 'medium' }",
                encoding="utf-8",
            )

            output_dir = root / "dist"
            payload_root = output_dir / "pauseink-v1.0.0-windows-x86_64"
            archive_path = output_dir / "pauseink-v1.0.0-windows-x86_64.zip"
            with chdir(root):
                package_release_asset.stage_payload(binary_path, payload_root)
                package_release_asset.create_zip(archive_path, payload_root)

            with zipfile.ZipFile(archive_path) as archive:
                names = set(archive.namelist())

            self.assertIn(
                "pauseink-v1.0.0-windows-x86_64/presets/style_presets/marker.json5",
                names,
            )
            self.assertIn(
                "pauseink-v1.0.0-windows-x86_64/presets/export_profiles/medium.json5",
                names,
            )


if __name__ == "__main__":
    unittest.main()
