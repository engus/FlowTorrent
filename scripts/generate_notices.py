"""Generate locked, target-specific third-party notice candidates."""
import argparse
import hashlib
from html import escape
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TOOL_VERSION = "0.9.2"
SLINT_LICENSE = "LicenseRef-Slint-Royalty-free-2.0"
SLINT_HASH = "5167f5056e850419106ab6265efbdca7cba4d99c849d1445ca0bbf6a1e2315fe"
SLINT_CRATES = {
    "i-slint-backend-selector", "i-slint-backend-winit", "i-slint-common", "i-slint-compiler",
    "i-slint-core", "i-slint-core-macros", "i-slint-renderer-software", "slint", "slint-build", "slint-macros",
}
TARGETS = ("aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-pc-windows-msvc")


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def complete_licenses(data):
    licenses = list(data["licenses"])
    if any(not item["text"].strip() for item in licenses):
        raise ValueError("Empty license text in notices output")
    covered = {user["crate"]["id"] for item in licenses for user in item["used_by"]}
    custom = None
    for item in data["crates"]:
        package = item["package"]
        if item["license"] == SLINT_LICENSE:
            if package["name"] not in SLINT_CRATES or package["version"] != "1.18.1":
                raise ValueError("Unreviewed Slint license/version: {}".format(package["name"]))
            path = Path(package["manifest_path"]).parent / "LICENSES" / (SLINT_LICENSE + ".md")
            if digest(path) != SLINT_HASH:
                raise ValueError("Slint license checksum changed; review the locked source")
            if package["id"] not in covered:
                # cargo-about0.9.2 can omit a clarified LicenseRef text while exiting zero.
                if custom is None:
                    custom = {"id": SLINT_LICENSE, "name": SLINT_LICENSE, "text": path.read_text(encoding="utf-8"), "used_by": []}
                    licenses.append(custom)
                custom["used_by"].append({"crate": package})
                covered.add(package["id"])
    missing = [item["package"]["name"] for item in data["crates"] if item["package"]["id"] not in covered]
    if missing:
        raise ValueError("Missing license text for: " + ", ".join(missing))
    return licenses


def render_notices(licenses):
    blocks = ["<!doctype html><html lang='en'><meta charset='utf-8'><title>FlowTorrent third-party notices</title>",
              "<main><h1>FlowTorrent third-party notices</h1><p>Generated from the locked normal/build dependency graph. "
              "Some build dependencies and supplemental license files are included conservatively. "
              "Each component retains its own license. FlowTorrent's own code is MIT.</p>"]
    for item in licenses:
        names = ", ".join(user["crate"]["name"] + " " + user["crate"]["version"] for user in item["used_by"])
        blocks.append("<section><h2>{}</h2><p>{}</p><pre style='white-space:pre-wrap'>{}</pre></section>".format(
            escape(item["name"]), escape(names), escape(item["text"])))
    blocks.append("</main></html>")
    return "\n".join(blocks)


def write_candidate(data, output, target, runtime_notice_dir=None):
    licenses = complete_licenses(data)
    output = Path(output)
    output.mkdir()
    try:
        (output / "THIRD_PARTY_NOTICES.html").write_text(render_notices(licenses), encoding="utf-8")
        if runtime_notice_dir is not None:
            runtime = Path(runtime_notice_dir)
            destination = output / "rust"; destination.mkdir()
            shutil.copy2(runtime / "COPYRIGHT-library.html", destination / "COPYRIGHT-library.html")
            shutil.copytree(runtime / "licenses", destination / "licenses", symlinks=True)
        for item in data["crates"]:
            package = item["package"]; source = Path(package["manifest_path"]).parent
            name = package["name"] + "-" + package["version"]
            paths = [source / "LICENSE"] if source.resolve() == ROOT.resolve() else source.rglob("*")
            for path in paths:
                if path.is_symlink() or not path.is_file() or not path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE", "UNLICENSE")):
                    continue
                relative = path.relative_to(source)
                destination = output / "license-files" / name / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, destination)
            if "MPL-2.0" in item["license"]:
                # Ship the unchanged component source, not just a link or a license identifier.
                shutil.copytree(source, output / "source" / name, symlinks=True)
        files = {str(path.relative_to(output).as_posix()): digest(path) for path in sorted(output.rglob("*")) if path.is_file()}
        manifest = {
            "schema": 1, "generator": "cargo-about " + TOOL_VERSION,
            "target": target, "cargo_lock_sha256": digest(ROOT / "Cargo.lock"),
            "policy_sha256": digest(ROOT / "about.toml"), "crate_count": len(data["crates"]),
            "crates": [{"name": x["package"]["name"], "version": x["package"]["version"], "license": x["license"], "source": x["package"]["source"]} for x in data["crates"]],
            "files": files,
        }
        (output / "NOTICE_MANIFEST.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    except BaseException:
        shutil.rmtree(output)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cargo-about", default=os.environ.get("CARGO_ABOUT", "cargo-about"))
    args = parser.parse_args()
    if args.output.exists() or args.output.is_symlink():
        parser.error("Use a fresh notices output directory")
    version = subprocess.check_output([args.cargo_about, "--version"], text=True).strip()
    if version != "cargo-about " + TOOL_VERSION:
        parser.error("Install cargo-about {} with --locked --features cli".format(TOOL_VERSION))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    sysroot = Path(subprocess.check_output([os.environ.get("RUSTC", "rustc"), "--print", "sysroot"], cwd=ROOT, text=True).strip())
    runtime = sysroot / "share/doc/rust"
    if not (runtime / "COPYRIGHT-library.html").is_file() or not (runtime / "licenses").is_dir():
        parser.error("Pinned Rust standard-library copyright/license files are required")
    with tempfile.TemporaryDirectory(prefix="flowtorrent-notices-") as temporary:
        raw = Path(temporary) / "licenses.json"
        subprocess.run([args.cargo_about, "generate", "--locked", "--fail", "--target", args.target,
                        "--config", str(ROOT / "about.toml"), "--manifest-path", str(ROOT / "Cargo.toml"),
                        "--format", "json", "--output-file", str(raw)], cwd=ROOT, check=True)
        write_candidate(json.loads(raw.read_text(encoding="utf-8")), args.output, args.target, runtime)
    print("Notices and source/license manifest: {}".format(args.output))


if __name__ == "__main__":
    main()
