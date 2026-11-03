"""Preservation at the package boundary and explicit profile provenance."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import distribution
import package_macos
import package_windows


class ProfilePackageTests(unittest.TestCase):
    def test_compact_binary_is_packaged_even_when_release_binary_exists(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for profile in ('release', 'compact'):
                binary = root / 'target/aarch64-apple-darwin' / profile / 'flowtorrent'
                binary.parent.mkdir(parents=True)
                binary.write_bytes(profile.encode())
            selected = distribution.profile_binary(root/'target', 'aarch64-apple-darwin', 'compact', 'flowtorrent')
            icon = root/'icon.icns'; icon.write_bytes(b'icon')
            app = root/'FlowTorrent.app'
            package_macos.assemble_bundle(selected, icon, app, '0.1.0')
            self.assertEqual((app/'Contents/MacOS/flowtorrent').read_bytes(), b'compact')
            with self.assertRaises(ValueError):
                distribution.profile_binary(root/'target', 'aarch64-apple-darwin', '../release', 'flowtorrent')

    def test_manifest_identifies_profile_and_compression_without_changing_test_status(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            artifact = root/'test.dmg'; artifact.write_bytes(b'package')
            lock = root/'Cargo.lock'; lock.write_bytes(b'locked')
            path = root/'test.json'
            distribution.write_manifest([artifact], path, '0.1.0', 'aarch64-apple-darwin', lock, 'revision',
                                        profile='compact', compression='ULMO')
            data = json.loads(path.read_text())
            self.assertEqual(data['build_profile'], 'compact')
            self.assertEqual(data['compression'], 'ULMO')
            self.assertFalse(data['release_ready'])
            self.assertEqual(data['artifacts'][0]['sha256'], hashlib.sha256(b'package').hexdigest())


@unittest.skipUnless(sys.platform == 'darwin', 'Native signed disk-image preservation requires macOS')
class NativeCompressionTests(unittest.TestCase):
    def test_ulmo_round_trip_preserves_every_signed_bundle_file(self):
        with tempfile.TemporaryDirectory(prefix='FlowTorrent compressed image ') as folder:
            root = Path(folder)
            binary = root/'flowtorrent'
            subprocess.run(['xcrun','clang','-mmacosx-version-min=14.0','-x','c','-o',str(binary),'-'],
                           input='int main(void) { return 0; }\n', text=True, check=True, capture_output=True)
            app = root/'FlowTorrent.app'
            package_macos.assemble_bundle(binary, package_macos.make_icon(root), app, '0.1.0')
            subprocess.run(['codesign','--force','--sign','-','--timestamp=none',str(app)],check=True,capture_output=True)
            expected = {p.relative_to(app).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                        for p in app.rglob('*') if p.is_file()}
            image = root/'compact.dmg'
            distribution.create_dmg(app, image, '0.1.0', image_format='ULMO')
            mount = root/'mounted'; mount.mkdir()
            subprocess.run(['hdiutil','attach','-readonly','-nobrowse','-noautoopen','-mountpoint',str(mount),str(image)],
                           check=True, capture_output=True)
            try:
                copied = mount/'FlowTorrent.app'
                actual = {p.relative_to(copied).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                          for p in copied.rglob('*') if p.is_file()}
                self.assertEqual(actual, expected)
                subprocess.run(['codesign','--verify','--strict',str(copied)],check=True,capture_output=True)
            finally:
                subprocess.run(['hdiutil','detach',str(mount)],check=True,capture_output=True)
