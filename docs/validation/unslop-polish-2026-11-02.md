[Русская версия](unslop-polish-2026-11-02-ru.md)

# Unslop desktop polish — 2 November 2026

## Candidate and scope

Branch `experiments/unslop-polish`, based on `10d647b` from `experiments/file-priorities`. The maintainer approved the three web studies and requested native main/settings/add polish with every existing function and behavior preserved. See the [approved design](../architecture/unslop-polish.md) and [plan](../superpowers/plans/2026-11-02-unslop-polish.md).

Presentation changes cover neutral surfaces, teal accents, stronger secondary contrast, sentence-case headings, 12–14 px content, 600 emphasis, 16 px action/state icons, quieter selection and aligned values. Torrent rows remain 64 px. The drawer keeps search, selection, file groups, priorities and actions. Settings retain every preference, autosave, focus and error route. Add retains both sources and the owned native window, with explicit selected size and wrapped final path. Standard inputs and checkboxes remain in place.

Handwritten Rust application/controller/engine/platform code, dependencies, manifest, storage and schemas are unchanged. Existing handlers, guards and accessibility actions remain authoritative. All 14 supported languages are retained; 189 contextual messages pass generator validation. Ordinary user settings/sessions, main, AppKit and other experiments were not modified.

## Automated evidence

Pinned Rust 1.92.0, Slint 1.18.1, `MACOSX_DEPLOYMENT_TARGET=14.0`, `--locked`, jobs 2. Final commands and logs are in ignored `_work/validation/unslop-polish-20261102/results.json` and adjacent files.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `python3 scripts/generate_translations.py --check` | Pass: 189 messages, 14 languages |
| `python3 -m unittest discover -s scripts/tests -v` | Pass: 49 entries, one Windows-only PowerShell check skipped |
| `cargo clippy --locked --all-targets -j 2 -- -D warnings` | Pass |
| `cargo test --locked --all-targets -j 2` | Pass: 201 tests, no failures or ignored tests |
| `cargo build --locked --example ui_smoke -j 2` | Pass |
| `cargo build --locked --release -j 2` | Existing host proc-macro failure E0463/E0432 for `zerofrom_derive` |
| Release with `--config 'profile.release.build-override.strip="none"'` | Pass; command-only workaround, no profile/dependency changes |
| `cargo run --locked --example macos_lifecycle_probe -j 2` | Pass: delegate identity, file callbacks, reopen, modal lease, hidden event loop |
| `git diff --check` | Pass |

Unchanged-base rendered priority tests passed before editing and after adapting their ink classifier to recognize teal as well as blue. Existing controller/settings/owned-window/guard/local-swarm tests pass.

A fresh read-only review found an invisible primary-button keyboard focus border, potential compact error-row overflow and German/Turkish capitalization mistakes. The visual issues were reproduced by failing rendered tests, fixed, then passed in isolation and the full suite. The primary now has a separate outer focus ring in both themes; zero vertical row padding leaves error text inside the 64 px row. Locales were corrected and regenerated. Native inspection also removed an unnecessary one-row file scrollbar by accounting for two border pixels.

## Native inspection

Built a fresh ad-hoc signed **debug test harness**, not a distributable/notarized application, from final `ui_smoke`. SHA-256: `404c084f776c2020d85020b39a805eb1c7bc026df96959df31853b0b4a2f48d0`. Bundle `FlowTorrent Unslop Final.app` and fixture `native-fixture-final` are in the ignored validation directory. Only the controlled loopback seeder and isolated test registry were used.

Observed on this Apple Silicon Mac:

- Main at 800×640 with open drawer, selection, file checkbox and both priority controls; wide main in dark English. The 524 KB fixture completed and seeded locally.
- Settings compact and expanded, RU/light and EN/dark, long folder path, theme/language persistence and keyboard focus. Page Up returns to the title; lower controls including About remain available by scrolling.
- Owned add at 640×560, expanded file list, wrapped full final path and selected size. Deselecting the last file shows zero bytes and disables Download; selecting it again enables confirmation. Confirming opens the completed torrent in main.
- Dark English magnet-source window retains disabled Continue for empty input; Escape returns to main.

Screenshots retained outside git beside the logs: `native-add-light.jpg`, `native-main-compact-light.jpg`, `native-main-wide-dark-en.jpg`, `native-settings-light.jpg`, `native-settings-dark-en.jpg`.

## Limits and integration

Add was inspected at its normal 640×560 size; direct physical resizing to its 480×360 minimum was not separately exercised. Automated owned-window and keyboard regressions pass. This is local Mac UI evidence, not a complete language/scale/renderer matrix, resource benchmark or physical Finder/Dock/tray acceptance. Windows x64 and Mac Intel CI/physical checks for this polish remain outstanding. Baseline branch CI is not claimed as evidence for these new sources.

The worktree and local branch are retained for review. No main merge, push, PR, release, tag, installer upload, signing-credential change or ordinary-app replacement was performed.

Method: [unslop](https://github.com/heydamir/unslop) by Damir Sokolovsky, CC BY 4.0, adapted to the existing native application.
