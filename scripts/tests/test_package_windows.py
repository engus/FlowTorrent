"""Portable checks at the executable boundary; native installer tests run on Windows."""
import importlib.util
import os
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("package_windows", ROOT / "scripts/package_windows.py")
packager = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(packager)


class WindowsToolTests(unittest.TestCase):
    def test_path_shim_does_not_hide_the_licensed_compiler_installation(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            shim = root / "chocolatey/bin/ISCC.exe"
            compiler = root / "Program Files/Inno Setup 6/ISCC.exe"
            rc = root / "sdk/rc.exe"
            for path in (shim, compiler, rc):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"tool fixture")
            (compiler.parent / "license.txt").write_text("Installed compiler license")
            with patch.dict(os.environ, {"ProgramFiles": str(root / "Program Files"), "ProgramFiles(x86)": str(root / "Program Files (x86)")}), patch.object(
                packager.shutil, "which", side_effect=lambda name: str(shim if name == "ISCC.exe" else rc)
            ):
                chosen, resource_compiler = packager.locate_tools(None)
            self.assertTrue(chosen.samefile(compiler))
            self.assertEqual(Path(resource_compiler), rc)

    def test_explicit_compiler_without_its_license_fails_before_building(self):
        with tempfile.TemporaryDirectory() as folder:
            compiler = Path(folder) / "ISCC.exe"; compiler.write_bytes(b"tool fixture")
            rc = Path(folder) / "rc.exe"; rc.write_bytes(b"tool fixture")
            with patch.object(packager.shutil, "which", return_value=str(rc)):
                with self.assertRaisesRegex(ValueError, "license"):
                    packager.locate_tools(str(compiler))

    def test_explicit_licensed_compiler_is_honored(self):
        with tempfile.TemporaryDirectory() as folder:
            compiler = Path(folder) / "ISCC.exe"; compiler.write_bytes(b"tool fixture")
            (compiler.parent / "license.txt").write_text("Installed compiler license")
            rc = Path(folder) / "rc.exe"; rc.write_bytes(b"tool fixture")
            with patch.object(packager.shutil, "which", return_value=str(rc)):
                chosen, _ = packager.locate_tools(str(compiler))
            self.assertTrue(chosen.samefile(compiler))


class WindowsExecutableTests(unittest.TestCase):
    def test_non_pe_payload_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "wrong.exe"; path.write_bytes(b"not a Windows executable")
            with self.assertRaises(ValueError):
                packager.verify_pe(path)

    def test_an_arm64_executable_cannot_be_labeled_x64(self):
        with tempfile.TemporaryDirectory() as folder:
            header = bytearray(0x86); header[:2] = b"MZ"
            header[0x3c:0x40] = struct.pack("<I", 0x80)
            header[0x80:0x86] = b"PE\0\0\x64\xaa"
            path = Path(folder) / "arm.exe"; path.write_bytes(header)
            with self.assertRaisesRegex(ValueError, "AMD64"):
                packager.verify_pe(path)
            header[0x84:0x86] = b"\x64\x86"; path.write_bytes(header)
            packager.verify_pe(path)

    def test_truncated_pe_pointer_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "short.exe"; path.write_bytes(b"MZ")
            with self.assertRaisesRegex(ValueError, "Truncated"):
                packager.verify_pe(path)


if __name__ == "__main__":
    unittest.main()
