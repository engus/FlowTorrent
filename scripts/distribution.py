"""Shared test-package filesystem operations."""
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tempfile


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def validate_version(version):
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("Packaging requires a numeric Cargo major.minor.patch version")


def artifact_records(artifacts):
    paths = sorted(map(Path, artifacts), key=lambda path: path.name)
    names = [path.name.casefold() for path in paths]
    if not paths or len(set(names)) != len(paths):
        raise ValueError("Artifacts need unique portable file names")
    records = []
    for path in paths:
        if path.is_symlink() or not path.is_file() or any(c in path.name for c in "\r\n\\"):
            raise ValueError("Artifact must be a regular file with a portable name: {}".format(path))
        records.append({"file": path.name, "bytes": path.stat().st_size, "sha256": sha256(path)})
    return records


def stage_macos_image(app, output, version):
    app, output = Path(app), Path(output)
    validate_version(version)
    if app.is_symlink() or not app.is_dir():
        raise ValueError("Use an existing non-symlink FlowTorrent app")
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    if not isinstance(info, dict) or any(info.get(key) != value for key, value in {
        "CFBundleIdentifier": "com.engus.flowtorrent",
        "CFBundleExecutable": "flowtorrent", "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version, "CFBundleVersion": version,
    }.items()):
        raise ValueError("Bundle identity/version does not match this FlowTorrent build")
    if not (app / "Contents/Resources/LOCAL_BUILD.txt").is_file():
        raise ValueError("Disk image input must be explicitly labeled as a local test build")
    output.mkdir()
    shutil.copytree(app, output / "FlowTorrent.app", symlinks=True)
    (output / "Applications").symlink_to("/Applications", target_is_directory=True)
    (output / "READ_ME_FIRST.txt").write_text(
        "FlowTorrent {} — local test build\n\n"
        "Drag FlowTorrent.app to Applications (or your personal Applications folder).\n"
        "Quit the previous version before replacing it. Replacing the application keeps\n"
        "your session and downloaded files. Removing the app keeps those files too.\n\n"
        "This test build is ad-hoc signed, not Developer ID signed or notarized.\n"
        "It is not the public 1.0 release. Do not change macOS security settings.\n"
        "The full release checklist, including real sleep/wake and overnight tests,\n"
        "must pass before distribution. Licenses are inside the application's Resources.\n".format(version),
        encoding="utf-8",
    )


def write_checksums(artifacts, output):
    records = artifact_records(artifacts)
    if any(Path(path).absolute() == Path(output).absolute() for path in artifacts):
        raise ValueError("Checksum output must not replace an artifact")
    with Path(output).open("x", encoding="utf-8", newline="\n") as destination:
        for record in records:
            destination.write("{}  {}\n".format(record["sha256"], record["file"]))


def write_manifest(artifacts, output, version, target, lockfile, revision):
    validate_version(version)
    manifest = {
        "schema": 1, "application_id": "com.engus.flowtorrent", "version": version,
        "target": target, "channel": "local-test", "release_ready": False,
        "signing": "ad-hoc" if target.endswith("apple-darwin") else "unsigned",
        "source_revision": revision, "cargo_lock_sha256": sha256(lockfile),
        "artifacts": artifact_records(artifacts),
    }
    with Path(output).open("x", encoding="utf-8", newline="\n") as destination:
        destination.write(json.dumps(manifest, indent=2) + "\n")


def create_dmg(app, output, version):
    output = Path(output)
    if output.exists() or output.is_symlink():
        raise FileExistsError(output)
    if output.suffix.lower() != ".dmg":
        raise ValueError("Disk image destination must end in .dmg")
    subprocess.run(["codesign", "--verify", "--strict", str(app)], check=True)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".flowtorrent-image-", dir=output.parent) as temporary:
        temporary = Path(temporary)
        stage_macos_image(app, temporary / "contents", version)
        image = temporary / "FlowTorrent.dmg"
        subprocess.run([
            "hdiutil", "create", "-volname", "FlowTorrent Test", "-fs", "HFS+",
            "-format", "UDZO", "-srcfolder", str(temporary / "contents"), str(image),
        ], check=True)
        subprocess.run(["hdiutil", "verify", str(image)], check=True)
        # Same-volume link publishes without replacing a concurrently created artifact.
        os.link(image, output)


def verify_notices(notices, target, lockfile):
    root = Path(notices)
    if root.is_symlink() or not root.is_dir():
        raise ValueError("Notices must be a non-symlink directory")
    actual_files = set()
    for path in root.rglob("*"):
        if path.is_symlink() or not (path.is_file() or path.is_dir()):
            raise ValueError("Notice tree must contain only regular files and directories")
        if path.is_file():
            actual_files.add(path.relative_to(root).as_posix())
    manifest = json.loads((root / "NOTICE_MANIFEST.json").read_text(encoding="utf-8"))
    if manifest["target"] != target or manifest["cargo_lock_sha256"] != sha256(lockfile) or manifest["policy_sha256"] != sha256(Path(lockfile).parent / "about.toml"):
        raise ValueError("Notices must match this target, Cargo.lock and license policy")
    if not {"THIRD_PARTY_NOTICES.html", "rust/COPYRIGHT-library.html"}.issubset(manifest["files"]):
        raise ValueError("Dependency or Rust standard-library notices are missing")
    if actual_files != set(manifest["files"]) | {"NOTICE_MANIFEST.json"}:
        raise ValueError("Notice manifest must cover every packaged file exactly")
    for name, fingerprint in manifest["files"].items():
        relative = Path(name)
        path = root / relative
        if relative.is_absolute() or ".." in relative.parts or "\\" in name or path.is_symlink() or not path.resolve().is_relative_to(root.resolve()):
            raise ValueError("Notice assets must stay inside their root")
        if sha256(path) != fingerprint:
            raise ValueError("Notice asset changed: " + name)
    return manifest
