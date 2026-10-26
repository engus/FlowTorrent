[Русский](2026-10-26-slint-parity-ci-ru.md)
# Slint mockup parity and full CI

The approved v42 prototype is the visual reference. Preserve existing engine behavior, platform safety and AppKit isolation. Do not publish or upload installers.

1. [x] Run Check with test_packages=true; the reusable package job must receive upload_artifacts=false. Inspect Windows checks, release compilation and disposable installer validation.
2. [x] Correct observed native differences: restore the sidebar brand/nav, compact aligned table and state/progress columns, quieter selection, matching dark surfaces, compact detail facts/file groups and pinned actions. Preserve keyboard/accessibility and responsive minimum size.
3. [x] Compare isolated native fixtures with the reference in RU/EN, light/dark, and narrow/wide windows. Verify the existing behavioral suite, packaging tests, Clippy, release build and Mac lifecycle probe.
4. [x] Commit and push fixes, run the full workflow on the final code revision, inspect results and skipped installer upload steps, and record exact evidence and remaining physical Windows gates.

Verified results: [layout parity and full CI](../../slint-parity-validation.md), code 53a41e8, successful run 40276244577. All three installer upload steps skipped; physical acceptance remains separate.

Baseline: e79e23e. Initial workflow: https://github.com/engus/FlowTorrent/actions/runs/40269171261. Visual baseline: target/slint-redesign-validation-20261026/native-light-files.png versus .superpowers/brainstorm/71364-1792932748/prototype-validated-preview.jpg. Visual changes require rendered comparison; no source-text styling assertions.
