"""Build and package the existing application for local Mac testing."""
from pathlib import Path
import argparse
import json
import os
import platform
import plistlib
import re
import shlex
import shutil
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
APP_ID = "com.engus.flowtorrent"


def assemble_bundle(binary, icon, output, version):
    binary, icon, output = Path(binary), Path(icon), Path(output)
    for source in (binary, icon, ROOT / "LICENSE"):
        if not source.is_file():
            raise FileNotFoundError(source)
    with (ROOT / "packaging/macos/Info.plist").open("rb") as source:
        info = plistlib.load(source)
    info["CFBundleShortVersionString"] = version
    info["CFBundleVersion"] = version
    output.mkdir()
    executable = output / "Contents/MacOS/flowtorrent"
    executable.parent.mkdir(parents=True)
    resources = output / "Contents/Resources"
    resources.mkdir()
    shutil.copy2(binary, executable)
    executable.chmod(0o755)
    shutil.copy2(icon, resources / "flow.icns")
    shutil.copy2(ROOT / "LICENSE", resources / "LICENSE")
    (resources / "LOCAL_BUILD.txt").write_text(
        "FlowTorrent local test build. Ad-hoc signed; not Developer ID signed or notarized.\n"
        "Third-party distribution notices and release validation remain pending.\n",
        encoding="utf-8",
    )
    (output / "Contents/Info.plist").write_bytes(plistlib.dumps(info, sort_keys=False))
    (output / "Contents/PkgInfo").write_bytes(b"APPL????")


def publish_bundle(staged, destination, replace=False):
    staged, destination = Path(staged), Path(destination)
    check_destination(destination, replace)
    if not destination.exists():
        staged.rename(destination)
        return
    backup_dir = Path(tempfile.mkdtemp(prefix=".flowtorrent-previous-", dir=destination.parent))
    previous = backup_dir / destination.name
    try:
        destination.rename(previous)
    except BaseException:
        backup_dir.rmdir()
        raise
    try:
        staged.rename(destination)
    except BaseException:
        try:
            previous.rename(destination)
        except OSError as error:
            raise RuntimeError("Previous application preserved at {}".format(previous)) from error
        backup_dir.rmdir()
        raise
    shutil.rmtree(backup_dir)


def check_destination(destination, replace):
    if destination.suffix.lower() != ".app" or destination.is_symlink():
        raise ValueError("Choose an .app destination that is not a symbolic link")
    if not destination.exists():
        return
    if not replace:
        raise FileExistsError("{} exists; quit the app and use --replace to rebuild".format(destination))
    try:
        info = plistlib.loads((destination / "Contents/Info.plist").read_bytes())
        owned = (
            isinstance(info, dict)
            and info.get("CFBundleIdentifier") == APP_ID
            and info.get("CFBundleExecutable") == "flowtorrent"
            and info.get("CFBundlePackageType") == "APPL"
        )
    except (OSError, ValueError, plistlib.InvalidFileException):
        owned = False
    if not owned:
        raise ValueError("Refusing to replace an unrelated directory: {}".format(destination))


def make_icon(directory):
    master = directory / "flow-1024.png"
    subprocess.run([
        "sips", "-s", "format", "png", "-z", "1024", "1024",
        str(ROOT / "assets/flow.svg"), "--out", str(master),
    ], check=True, stdout=subprocess.DEVNULL)
    iconset = directory / "flow.iconset"
    iconset.mkdir()
    for size in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            suffix = "@2x" if scale == 2 else ""
            output = iconset / "icon_{}x{}{}.png".format(size, size, suffix)
            pixels = str(size * scale)
            subprocess.run([
                "sips", "-z", pixels, pixels, str(master), "--out", str(output),
            ], check=True, stdout=subprocess.DEVNULL)
    icon = directory / "flow.icns"
    subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(icon)], check=True)
    return icon


def verify_binary(binary, target):
    architecture = "arm64" if target == "aarch64-apple-darwin" else "x86_64"
    subprocess.run(["lipo", "-verify_arch", architecture, str(binary)], check=True)
    commands = subprocess.check_output(["otool", "-l", str(binary)], text=True)
    minimums = [tuple(map(int, value)) for value in re.findall(r"\bminos (\d+)\.(\d+)", commands)]
    if not minimums or max(minimums) > (14, 0):
        raise ValueError("The executable must support macOS 14.0")
    dependencies = subprocess.check_output(["otool", "-L", str(binary)], text=True)
    for line in dependencies.splitlines()[1:]:
        dependency = line.strip().split(" (", 1)[0]
        if not dependency.startswith(("/usr/lib/", "/System/Library/")):
            raise ValueError("Unbundled non-system dependency: {}".format(dependency))


def main():
    native = "aarch64-apple-darwin" if platform.machine() == "arm64" else "x86_64-apple-darwin"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=("aarch64-apple-darwin", "x86_64-apple-darwin"), default=native)
    parser.add_argument("--output", type=Path, help="destination .app (default: target/local-macos/<target>/FlowTorrent.app)")
    parser.add_argument("--replace", action="store_true", help="replace a previous FlowTorrent bundle; quit it first")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("Mac packaging requires macOS and Xcode Command Line Tools")
    try:
        for tool in ("sips", "iconutil", "codesign", "plutil", "lipo", "otool"):
            if shutil.which(tool) is None:
                raise ValueError("Required macOS tool is unavailable: {}".format(tool))
        cargo = os.environ.get("CARGO", "cargo")
        environment = dict(os.environ, MACOSX_DEPLOYMENT_TARGET="14.0")
        metadata = json.loads(subprocess.check_output([
            cargo, "metadata", "--locked", "--no-deps", "--format-version", "1",
        ], cwd=ROOT, env=environment, text=True))
        package = next(package for package in metadata["packages"] if package["name"] == "flowtorrent")
        version = package["version"]
        output = (args.output or ROOT / "target/local-macos" / args.target / "FlowTorrent.app").absolute()
        check_destination(output, args.replace)
        print("Building FlowTorrent {} for {} (local test)".format(version, args.target), flush=True)
        # rustc's stripping can create proc-macro dylibs rejected by macOS 27.
        # Restrict the workaround to this Mac command's host build dependencies;
        # keep the executable's optimized/stripped profile and Windows unchanged.
        subprocess.run([
            cargo, "build", "--locked", "--release", "--bin", "flowtorrent",
            "--target", args.target, "-j", "2",
            "--config", 'profile.release.build-override.strip="none"',
        ], cwd=ROOT, env=environment, check=True)
        binary = Path(metadata["target_directory"]) / args.target / "release/flowtorrent"
        verify_binary(binary, args.target)
        output.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix=".flowtorrent-package-", dir=output.parent) as temporary:
            directory = Path(temporary)
            icon = make_icon(directory)
            staged = directory / "FlowTorrent.app"
            assemble_bundle(binary, icon, staged, version)
            subprocess.run(["plutil", "-lint", str(staged / "Contents/Info.plist")], check=True)
            subprocess.run(["codesign", "--force", "--sign", "-", "--timestamp=none", str(staged)], check=True)
            subprocess.run(["codesign", "--verify", "--strict", str(staged)], check=True)
            publish_bundle(staged, output, args.replace)
        print("Local test app: {}".format(output))
        print("Open with: open {}".format(shlex.quote(str(output))))
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        parser.exit(1, "Mac packaging failed: {}\n".format(error))


if __name__ == "__main__":
    main()
