"""Artifact contents and preservation rules, using real temporary files."""
import importlib.util
import json
from pathlib import Path
import plistlib
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("distribution", Path(__file__).resolve().parents[1] / "distribution.py")
distribution = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(distribution)


class DistributionTests(unittest.TestCase):
    def test_asset_notices_copy_complete_icon_and_font_licenses(self):
        root = Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder)
            distribution.copy_asset_licenses(root, output)
            self.assertEqual((output / "Lucide-LICENSE.txt").read_bytes(), (root / "assets/icons/lucide/LICENSE").read_bytes())
            self.assertEqual((output / "Nunito-OFL.txt").read_bytes(), (root / "assets/brand/OFL.txt").read_bytes())

    def test_missing_asset_license_is_detected_before_copying_anything(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "assets/brand").mkdir(parents=True)
            (root / "assets/brand/OFL.txt").write_text("font license")
            output = root / "output"; output.mkdir()
            with self.assertRaises(FileNotFoundError):
                distribution.copy_asset_licenses(root, output)
            self.assertEqual(list(output.iterdir()), [])

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.app = self.root / "FlowTorrent.app"
        resources = self.app / "Contents/Resources"
        resources.mkdir(parents=True)
        (resources / "LOCAL_BUILD.txt").write_text("Local test build")
        (self.app / "Contents/Info.plist").write_bytes(plistlib.dumps({
            "CFBundleIdentifier": "com.engus.flowtorrent",
            "CFBundleExecutable": "flowtorrent", "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": "0.1.0", "CFBundleVersion": "0.1.0",
        }))
        executable = self.app / "Contents/MacOS/flowtorrent"
        executable.parent.mkdir()
        executable.write_bytes(b"owned executable")

    def notices_fixture(self):
        notices = self.root / "notices"; notices.mkdir()
        (notices / "THIRD_PARTY_NOTICES.html").write_bytes(b"abc")
        (notices / "rust").mkdir()
        (notices / "rust/COPYRIGHT-library.html").write_bytes(b"abc")
        lock = self.root / "Cargo.lock"; lock.write_bytes(b"abc")
        (self.root / "about.toml").write_bytes(b"abc")
        fingerprint = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        manifest = {"target": "aarch64-apple-darwin", "cargo_lock_sha256": fingerprint,
                    "policy_sha256": fingerprint, "files": {"THIRD_PARTY_NOTICES.html": fingerprint, "rust/COPYRIGHT-library.html": fingerprint}}
        (notices / "NOTICE_MANIFEST.json").write_text(json.dumps(manifest))
        return notices, lock

    def test_notice_verification_accepts_exact_target_lock_policy_and_bytes(self):
        notices, lock = self.notices_fixture()
        manifest = distribution.verify_notices(notices, "aarch64-apple-darwin", lock)
        self.assertIsInstance(manifest, dict)
        lock.write_bytes(b"changed dependencies")
        with self.assertRaises(ValueError):
            distribution.verify_notices(notices, "aarch64-apple-darwin", lock)

    def test_missing_standard_library_notices_are_rejected(self):
        notices, lock = self.notices_fixture()
        path = notices / "NOTICE_MANIFEST.json"; manifest = json.loads(path.read_text())
        del manifest["files"]["rust/COPYRIGHT-library.html"]
        path.write_text(json.dumps(manifest))
        with self.assertRaises(ValueError):
            distribution.verify_notices(notices, "aarch64-apple-darwin", lock)

    def test_unlisted_notice_file_cannot_enter_a_package(self):
        notices, lock = self.notices_fixture()
        (notices / "rust/unlisted.txt").write_text("not covered by the manifest")
        with self.assertRaises(ValueError):
            distribution.verify_notices(notices, "aarch64-apple-darwin", lock)

    def test_unlisted_notice_links_cannot_enter_a_package(self):
        notices, lock = self.notices_fixture()
        for destination in (lock, self.root):
            with self.subTest(destination=destination):
                link = notices / "unlisted"
                try:
                    link.symlink_to(destination, target_is_directory=destination.is_dir())
                except OSError:
                    self.skipTest("This Windows account cannot create symbolic links")
                try:
                    with self.assertRaises(ValueError):
                        distribution.verify_notices(notices, "aarch64-apple-darwin", lock)
                finally:
                    link.unlink()

    def test_notice_manifest_itself_cannot_be_a_link(self):
        notices, lock = self.notices_fixture()
        manifest = notices / "NOTICE_MANIFEST.json"
        outside = self.root / "manifest.json"
        manifest.rename(outside)
        try:
            manifest.symlink_to(outside)
        except OSError:
            self.skipTest("This Windows account cannot create symbolic links")
        with self.assertRaises(ValueError):
            distribution.verify_notices(notices, "aarch64-apple-darwin", lock)

    def test_notices_reject_wrong_platform_and_changed_license_files(self):
        notices, lock = self.notices_fixture()
        with self.assertRaises(ValueError):
            distribution.verify_notices(notices, "x86_64-pc-windows-msvc", lock)
        (notices / "THIRD_PARTY_NOTICES.html").write_text("modified")
        with self.assertRaises(ValueError):
            distribution.verify_notices(notices, "aarch64-apple-darwin", lock)

    def test_notice_manifest_cannot_reference_files_outside_its_root(self):
        notices, lock = self.notices_fixture()
        path = notices / "NOTICE_MANIFEST.json"; manifest = json.loads(path.read_text())
        manifest["files"]["../Cargo.lock"] = manifest["cargo_lock_sha256"]
        path.write_text(json.dumps(manifest))
        with self.assertRaises(ValueError):
            distribution.verify_notices(notices, "aarch64-apple-darwin", lock)

    def test_disk_image_staging_keeps_app_and_test_identity(self):
        image = self.root / "image"
        distribution.stage_macos_image(self.app, image, "0.1.0")
        self.assertTrue((image / "Applications").is_symlink())
        self.assertEqual((image / "Applications").readlink(), Path("/Applications"))
        self.assertEqual((image / "FlowTorrent.app/Contents/MacOS/flowtorrent").read_bytes(), b"owned executable")
        self.assertTrue((self.app / "Contents/MacOS/flowtorrent").exists())
        self.assertIn("test build", (image / "READ_ME_FIRST.txt").read_text().lower())

    def test_staging_rejects_wrong_version_before_creating_image(self):
        image = self.root / "image"
        with self.assertRaises(ValueError):
            distribution.stage_macos_image(self.app, image, "1.0.0")
        self.assertFalse(image.exists())

    def test_staging_refuses_unrelated_or_symlink_application(self):
        info = self.app / "Contents/Info.plist"
        data = plistlib.loads(info.read_bytes()); data["CFBundleIdentifier"] = "another.app"
        info.write_bytes(plistlib.dumps(data))
        with self.assertRaises(ValueError):
            distribution.stage_macos_image(self.app, self.root / "image", "0.1.0")
        link = self.root / "linked.app"
        try:
            link.symlink_to(self.app, target_is_directory=True)
        except OSError:
            return  # Windows accounts without symlink privilege still test the identity guard.
        with self.assertRaises(ValueError):
            distribution.stage_macos_image(link, self.root / "image", "0.1.0")

    def test_existing_image_staging_is_never_erased(self):
        image = self.root / "image"; image.mkdir()
        (image / "keep").write_text("keep")
        with self.assertRaises(FileExistsError):
            distribution.stage_macos_image(self.app, image, "0.1.0")
        self.assertEqual((image / "keep").read_text(), "keep")

    def test_existing_disk_image_is_not_overwritten(self):
        output = self.root / "FlowTorrent.dmg"; output.write_bytes(b"keep existing image")
        with self.assertRaises(FileExistsError):
            distribution.create_dmg(self.app, output, "0.1.0")
        self.assertEqual(output.read_bytes(), b"keep existing image")

    def test_checksums_cover_artifact_bytes_with_portable_names(self):
        artifact = self.root / "FlowTorrent-test.dmg"; artifact.write_bytes(b"abc")
        output = self.root / "SHA256SUMS"
        distribution.write_checksums([artifact], output)
        self.assertTrue(output.exists(), "checksum file must be written")
        self.assertEqual(output.read_text(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  FlowTorrent-test.dmg\n")

    def test_checksums_refuse_to_replace_existing_evidence(self):
        artifact = self.root / "app.exe"; artifact.write_bytes(b"abc")
        output = self.root / "SHA256SUMS"; output.write_text("previous evidence")
        with self.assertRaises(FileExistsError):
            distribution.write_checksums([artifact], output)
        self.assertEqual(output.read_text(), "previous evidence")

    def test_checksums_reject_ambiguous_artifact_names(self):
        first = self.root / "same.exe"; first.write_bytes(b"a")
        folder = self.root / "other"; folder.mkdir()
        second = folder / "same.exe"; second.write_bytes(b"b")
        with self.assertRaises(ValueError):
            distribution.write_checksums([first, second], self.root / "SHA256SUMS")

    def test_manifest_records_bytes_hash_lock_and_test_only_status(self):
        artifact = self.root / "app.dmg"; artifact.write_bytes(b"abc")
        lock = self.root / "Cargo.lock"; lock.write_bytes(b"locked")
        output = self.root / "manifest.json"
        distribution.write_manifest([artifact], output, "0.1.0", "aarch64-apple-darwin", lock, "abc123")
        self.assertTrue(output.exists(), "manifest must be written alongside artifacts")
        manifest = json.loads(output.read_text())
        self.assertEqual(manifest["channel"], "local-test")
        self.assertFalse(manifest["release_ready"])
        self.assertEqual(manifest["version"], "0.1.0")
        self.assertEqual(manifest["artifacts"][0]["bytes"], 3)
        self.assertEqual(manifest["artifacts"][0]["sha256"], "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")


if __name__ == "__main__":
    unittest.main()
