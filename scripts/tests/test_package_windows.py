"""Portable checks at the executable boundary; native installer tests run on Windows."""
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("package_windows", ROOT / "scripts/package_windows.py")
packager = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(packager)


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
