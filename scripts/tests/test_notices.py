import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("generate_notices", Path(__file__).resolve().parents[1] / "generate_notices.py")
notices = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(notices)


class NoticeTests(unittest.TestCase):
    def test_workspace_notices_do_not_copy_build_output_or_user_data(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder) / "project"; root.mkdir()
            (root / "LICENSE").write_text("Project MIT text")
            (root / "Cargo.lock").write_text("locked")
            (root / "about.toml").write_text("policy")
            generated = root / "target/user-data"; generated.mkdir(parents=True)
            (generated / "NOTICE").write_text("Do not collect build output or user files")
            package = {"id": "path#flowtorrent@0.1.0", "name": "flowtorrent", "version": "0.1.0", "manifest_path": str(root / "Cargo.toml"), "source": None}
            data = {"licenses": [{"id": "MIT", "name": "MIT", "text": "MIT text", "used_by": [{"crate": package}]}], "crates": [{"package": package, "license": "MIT"}]}
            output = Path(folder) / "notices"
            with patch.object(notices, "ROOT", root):
                notices.write_candidate(data, output, "aarch64-apple-darwin")
            manifest = json.loads((output / "NOTICE_MANIFEST.json").read_text())
            self.assertFalse(any("/target/" in name for name in manifest["files"]), "workspace collection must not crawl generated output")

    def test_nested_desktop_collects_root_license_without_crawling_package_data(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)/"project"; root.mkdir()
            (root/"LICENSE").write_text("Author MIT text")
            (root/"Cargo.lock").write_text("locked")
            (root/"about.toml").write_text("policy")
            source = root/"apps/desktop"; source.mkdir(parents=True)
            (source/"NOTICE-user-data").write_text("Do not distribute")
            package = {"id": "path#flowtorrent@0.1.0", "name": "flowtorrent", "version": "0.1.0", "manifest_path": str(source/"Cargo.toml"), "source": None}
            data = {"licenses": [{"id": "MIT", "name": "MIT", "text": "MIT text", "used_by": [{"crate": package}]}], "crates": [{"package": package, "license": "MIT"}]}
            output = Path(folder)/"notices"
            with patch.object(notices, "ROOT", root):
                notices.write_candidate(data, output, "aarch64-apple-darwin")
            files = list((output/"license-files/flowtorrent-0.1.0").iterdir())
            self.assertEqual([p.name for p in files], ["LICENSE"])
            self.assertEqual(files[0].read_text(), "Author MIT text")

    def test_successful_tool_output_with_unrepresented_crate_is_rejected(self):
        package = {"id": "registry#missing@1.0.0", "name": "missing", "version": "1.0.0"}
        with self.assertRaisesRegex(ValueError, "missing"):
            notices.complete_licenses({"licenses": [], "crates": [{"package": package, "license": "MIT"}]})

    def test_empty_license_text_does_not_count_as_coverage(self):
        package = {"id": "registry#sample@1.0.0", "name": "sample", "version": "1.0.0"}
        data = {"licenses": [{"id": "MIT", "text": "", "used_by": [{"crate": package}]}], "crates": [{"package": package, "license": "MIT"}]}
        with self.assertRaises(ValueError):
            notices.complete_licenses(data)

    def test_supplemented_custom_license_requires_locked_source_checksum(self):
        with tempfile.TemporaryDirectory() as folder:
            source = Path(folder) / "LICENSES/LicenseRef-Slint-Royalty-free-2.0.md"
            source.parent.mkdir(); source.write_text("changed or unrelated license")
            package = {"id": "registry#slint@1.18.1", "name": "slint", "version": "1.18.1", "manifest_path": str(Path(folder) / "Cargo.toml")}
            data = {"licenses": [], "crates": [{"package": package, "license": "LicenseRef-Slint-Royalty-free-2.0"}]}
            with self.assertRaisesRegex(ValueError, "checksum"):
                notices.complete_licenses(data)

    def test_html_escapes_license_text_and_has_no_executable_markup_from_input(self):
        licenses = [{"id": "MIT", "name": "MIT", "text": "<script>bad()</script>", "used_by": [{"crate": {"name": "example", "version": "1.0.0"}}]}]
        html = notices.render_notices(licenses)
        self.assertIsInstance(html, str)
        self.assertIn("&lt;script&gt;bad()&lt;/script&gt;", html)
        self.assertNotIn("<script>", html)


if __name__ == "__main__":
    unittest.main()
