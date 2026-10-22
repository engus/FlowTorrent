"""Keep the documented ringbuf advisory exception confined to its audited graph."""
import argparse
import json
from pathlib import Path


def check_scope(metadata):
    packages = {package["id"]: package for package in metadata["packages"]}
    buffers = [package for package in packages.values() if package["name"] == "ringbuf"]
    registry = "registry+https://github.com/rust-lang/crates.io-index"
    if len(buffers) != 1 or buffers[0]["version"] != "0.4.8" or buffers[0].get("source") != registry:
        raise ValueError("ringbuf version/source changed; remove or reassess the exception")
    buffer_id = buffers[0]["id"]
    consumers = [packages[node["id"]] for node in metadata["resolve"]["nodes"] if buffer_id in node["dependencies"]]
    if len(consumers) != 1 or consumers[0]["name"] != "librqbit-utp" or consumers[0]["version"] != "0.7.0" or consumers[0].get("source") != registry:
        raise ValueError("ringbuf has a different consumer; recheck element types and panic safety")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("metadata", type=Path, help="cargo metadata --locked --format-version1 JSON")
    args = parser.parse_args()
    try:
        check_scope(json.loads(args.metadata.read_text(encoding="utf-8")))
    except (OSError, ValueError, KeyError) as error:
        parser.exit(1, "Reassess RUSTSEC-2026-0293 before accepting this dependency graph: {}\n".format(error))
    print("ringbuf exception scope matches the audited byte-buffer dependency")


if __name__ == "__main__":
    main()
