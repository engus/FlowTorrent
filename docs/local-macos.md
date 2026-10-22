# Local Mac development

The local bundle uses the existing Rust/Slint application and shared torrent
engine. It supports macOS 14.0 or later. Windows source, dependencies and build
profiles remain shared and unchanged by the bundler.

## Build and run

Requirements: macOS, Xcode Command Line Tools (`xcode-select --install` if needed),
Rust installed through rustup, and Python 3.9 or later. The repository pins Rust
1.92.0. No Python packages or Apple signing account are required.

From the repository root:

```sh
./scripts/package-macos.sh
```

The command prints the app path and an `open` command. The default destination
is `target/local-macos/<target>/FlowTorrent.app`; `<target>` is
`aarch64-apple-darwin` on Apple Silicon or `x86_64-apple-darwin` on Intel.
The app can also be opened from Finder or copied to a location of your choice.

Quit FlowTorrent before rebuilding an existing destination:

```sh
./scripts/package-macos.sh --replace
```

To choose a different output location:

```sh
./scripts/package-macos.sh --output "$HOME/Applications/FlowTorrent.app"
```

An existing app requires `--replace`; the command refuses unrelated directories
and symbolic-link destinations. It assembles and verifies the new app before
replacing the previous bundle, restoring the previous one if publication fails.
Builds use the locked dependencies and two Cargo jobs. The macOS packaging
command disables stripping for host build dependencies to avoid malformed
proc-macro dylibs on macOS 27; the application still uses its optimized release
profile.

For an Intel app, install the target first, then select it explicitly:

```sh
rustup target add x86_64-apple-darwin
./scripts/package-macos.sh --target x86_64-apple-darwin
```

This creates separate architecture-specific apps. Run an Intel build on an Intel
Mac for native validation. The manual **Local Mac app** GitHub workflow builds
both architectures on their matching runners and saves ZIP artifacts, without
publishing a release. It must be run from a revision containing these scripts.

## Local build status

The bundle identity is `com.engus.flowtorrent`, with its version read from
`Cargo.toml`. It contains the executable, an icon, the project license and a
local-build notice. The bundler checks architecture, macOS deployment target,
system library dependencies, plist syntax and the ad-hoc code signature.

This is a local test build. Developer ID signing, notarization, a DMG, complete
third-party distribution notices and the remaining release gates are still
pending. The bundle registers `.torrent` support with alternate handler rank;
it does not set itself as the default application. To test a file explicitly,
use Finder's **Open With → FlowTorrent**, or:

```sh
open -a /absolute/path/to/FlowTorrent.app /absolute/path/to/example.torrent
```

The app uses normal macOS application-support and preference locations. Replacing
the `.app` preserves its saved session and downloaded files. Tests must not
delete existing user state. Packaging does not resolve the shared engine and
session issues documented by the project audit; use this preview with test data.

The split **Add torrent | magnet icon** control is available in the toolbar and
empty view. Add torrent opens the local file picker. The magnet icon opens a link
field; Continue fetches metadata before showing the normal file/destination
preview. Cancel stops lookup; lookup also has a two-minute timeout. Supported
links contain a BitTorrent v1 hash (including hybrid links). Paste links into the
app; system-wide magnet URL registration is not included. Optional selection
hints in links are ignored so selection remains explicit in the preview.

## Verification

Portable packaging checks (also included in Windows and Mac CI):

```sh
python3 -m unittest discover -s scripts/tests -v
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

For each locally built app, check:

1. Open from Finder; the Dock icon and menu bar controls appear.
2. Open a harmless `.torrent` file while running; check preview and cancel.
3. Quit, then open that file with FlowTorrent; check preview and cancel again.
4. Close the window, acknowledge the background notice, and reopen the app.
   The same process should show its window.
5. Quit using the app menu or Command-Q; verify the process exits.

Native notifications require the bundle identity and permission from macOS.
Completion delivery, real transfers, sleep/wake and overnight operation need
separate runtime validation beyond building the app.

### Verified locally on 2026-10-22

FlowTorrent 0.1.0 was built on Apple Silicon with macOS 27.0.1, Xcode 27.1
and Rust 1.92.0. The delivered app is at
`/Users/yevgeniygolota/Documents/Projects/FlowTorrent/target/local-macos/FlowTorrent.app`.

- Release build, plist validation and strict signature verification passed.
  `lipo` reports arm64; `LC_BUILD_VERSION` reports `minos 14.0`.
- The app launched and displayed its icon and empty download view.
- Finder **Open With → FlowTorrent** delivered a synthetic 256-byte torrent to
  its preview, both while running and from a closed app. Cancel returned to the
  empty view without adding a download. Transmission remained the default.
- Closing displayed the first-use background notice. The process remained
  running; reopening restored the main window with the same PID (19682).
- Command-Q displayed shutdown progress and exited. The native app menu includes
  About, Hide and Quit. The final app was reopened and left running for testing.
- Six Python packaging tests, formatting, Clippy with warnings denied, and all
  86 Rust test entries passed. A simulated Windows `stat` check also passed after
  restricting the executable-bit assertion to Unix platforms.

No Intel or Windows runtime test was performed locally, and the new packaging
workflow has not been run on GitHub. macOS 14 support is checked through build
metadata; runtime testing here used macOS 27. Notifications, completed transfers,
sleep/wake and overnight behavior remain unverified in this bundle.

Independent review found no remaining important source blocker. One minor guard
weakness remains: deployment-version verification compares major/minor only,
so a future `minos 14.0.1` would be accepted. This artifact reports exactly 14.0.

### Add and magnet update (2026-10-22)

The same local app path now contains the split Add torrent / magnet control.
The final build was launched and inspected. Add torrent opened the renamed file
picker, selected the local synthetic torrent and reached the existing preview;
Cancel returned to the empty list. The magnet pane showed an inline error for an
HTTP URL and fetching progress for a synthetic magnet, and Cancel stopped the
lookup. No payload was added to the user's downloads.

Validation passed: 94 application Rust test entries, 18 bencode decoder tests,
six packaging tests, formatting, Clippy with warnings denied, and native bundle
signing/verification. The new Windows/macOS CI step also runs the decoder tests;
CI has not yet been run for this uncommitted revision.

The regression suite uses a loopback seeder to resolve magnet metadata, confirm
the normal preview and verify the downloaded bytes. It also checks cancelling
lookup before queued local files, quitting during lookup, duplicate-torrent UI
focus and deeply nested metadata from a controlled BEP 9 peer. The narrowly
patched decoder and its maintenance notes are in
`vendor/librqbit-bencode/PATCHES.md`.

Keyboard clipboard paste and screen-reader behavior still need a physical-device
check: the automation's paste shortcut inserted a literal character, so the
remaining entry checks used accessibility text setting. No custom clipboard
implementation was added. System-wide magnet associations remain unchanged.
