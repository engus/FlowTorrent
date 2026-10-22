"""Filesystem behavior of the local packager; native signing is verified on Mac."""
import importlib.util
from pathlib import Path
import plistlib
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("package_macos", ROOT / "scripts/package_macos.py")
packager = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(packager)


class BundleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = self.root / "flowtorrent"
        self.binary.write_bytes(b"test executable bytes")
        self.binary.chmod(0o755)
        self.icon = self.root / "flow.icns"
        self.icon.write_bytes(b"test icon bytes")

    def bundle(self, name, version="0.1.0"):
        output = self.root / name
        packager.assemble_bundle(self.binary, self.icon, output, version)
        self.assertTrue(output.is_dir(), "assembler must create an application bundle")
        return output

    def test_bundle_contains_versioned_identity_executable_icon_and_viewer(self):
        output = self.bundle("Проверка с пробелами.app", "0.2.3")
        executable = output / "Contents/MacOS/flowtorrent"
        self.assertEqual(executable.read_bytes(), b"test executable bytes")
        if sys.platform != "win32":
            self.assertTrue(executable.stat().st_mode & stat.S_IXUSR)
        info = plistlib.loads((output / "Contents/Info.plist").read_bytes())
        self.assertEqual(info["CFBundleIdentifier"], "com.engus.flowtorrent")
        self.assertEqual(info["CFBundleExecutable"], "flowtorrent")
        self.assertEqual(info["CFBundleShortVersionString"], "0.2.3")
        self.assertEqual(info["CFBundleVersion"], "0.2.3")
        self.assertEqual(info["LSMinimumSystemVersion"], "14.0")
        self.assertEqual(info["CFBundlePackageType"], "APPL")
        document = info["CFBundleDocumentTypes"][0]
        self.assertEqual(document["CFBundleTypeRole"], "Viewer")
        self.assertEqual(document["LSHandlerRank"], "Alternate")
        self.assertIn("org.bittorrent.torrent", document["LSItemContentTypes"])
        self.assertEqual((output / "Contents/Resources/flow.icns").read_bytes(), b"test icon bytes")
        self.assertTrue((output / "Contents/Resources/LICENSE").is_file())

    def test_patch_level_newer_than_supported_macos_is_rejected(self):
        commands = "Load command 1\n cmd LC_BUILD_VERSION\n platform 1\n minos 14.0.1\n sdk 27.0\n"
        dependencies = "flowtorrent:\n /usr/lib/libSystem.B.dylib (compatibility version 1.0.0)\n"
        with patch.object(packager.subprocess, "run"), patch.object(
            packager.subprocess, "check_output", side_effect=[commands, dependencies]
        ):
            with self.assertRaisesRegex(ValueError, "macOS 14.0"):
                packager.verify_binary(self.binary, "aarch64-apple-darwin")

    def test_missing_input_does_not_leave_a_partial_bundle(self):
        output = self.root / "FlowTorrent.app"
        with self.assertRaises(FileNotFoundError):
            packager.assemble_bundle(self.root / "missing", self.icon, output, "0.1.0")
        self.assertFalse(output.exists())

    def test_existing_output_is_preserved_without_explicit_replace(self):
        existing = self.bundle("FlowTorrent.app")
        sentinel = existing / "keep.txt"
        sentinel.write_text("keep")
        staged = self.bundle("staged.app", "0.2.0")
        with self.assertRaises(FileExistsError):
            packager.publish_bundle(staged, existing)
        self.assertEqual(sentinel.read_text(), "keep")
        self.assertTrue(staged.is_dir())

    def test_replace_refuses_unrelated_directory_and_symlink(self):
        unrelated = self.root / "Other.app"
        unrelated.mkdir()
        sentinel = unrelated / "keep.txt"
        sentinel.write_text("keep")
        staged = self.bundle("staged.app")
        with self.assertRaises(ValueError):
            packager.publish_bundle(staged, unrelated, replace=True)
        self.assertEqual(sentinel.read_text(), "keep")
        link = self.root / "Linked.app"
        try:
            link.symlink_to(unrelated, target_is_directory=True)
        except OSError:
            self.skipTest("symlink creation is not available on this host")
        with self.assertRaises(ValueError):
            packager.publish_bundle(staged, link, replace=True)
        self.assertTrue(link.is_symlink())
        self.assertTrue(staged.is_dir())

    def test_explicit_replace_publishes_new_bundle_and_removes_old_artifact(self):
        existing = self.bundle("FlowTorrent.app")
        (existing / "old-marker").write_text("old")
        staged = self.bundle("staged.app", "0.2.0")
        packager.publish_bundle(staged, existing, replace=True)
        self.assertFalse(staged.exists())
        self.assertFalse((existing / "old-marker").exists())
        info = plistlib.loads((existing / "Contents/Info.plist").read_bytes())
        self.assertEqual(info["CFBundleVersion"], "0.2.0")

    def test_failed_publish_restores_previous_application(self):
        existing = self.bundle("FlowTorrent.app")
        (existing / "keep.txt").write_text("previous app")
        staged = self.bundle("staged.app", "0.2.0")
        rename = Path.rename

        def fail_new_bundle(path, target):
            if path == staged:
                raise OSError("simulated publication failure")
            return rename(path, target)

        with patch.object(Path, "rename", fail_new_bundle):
            with self.assertRaises(OSError):
                packager.publish_bundle(staged, existing, replace=True)
        self.assertEqual((existing / "keep.txt").read_text(), "previous app")
        self.assertTrue(staged.is_dir())


if __name__ == "__main__":
    unittest.main()
