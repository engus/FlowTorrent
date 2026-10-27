[Русская версия](windows-workspace-packaging-ru.md)

# Windows package selection after the workspace move

In [the first full CI run](https://github.com/engus/FlowTorrent/actions/runs/40516333124), Windows application tests, Clippy and license coverage passed. Installer creation stopped before compilation: Cargo rejected the virtual root manifest for `cargo rustc --bin flowtorrent`. Unlike the package builder's `cargo build`, this command needs an explicit package selection at a virtual workspace root.

The resource-link build command now selects `--package flowtorrent`, preserving the locked release build, Windows target, two jobs, static CRT and compiled icon/version resource. The test executes the actual command against a dependency-free virtual workspace, adding `--print=cfg` to validate package and target selection without a Windows SDK or linking. It uses Rust 1.92.0 and offline mode. It reproduced the exact virtual-manifest error before the fix, then passed; all 49 script tests passed locally.

No Rust engine/UI, AppKit prototype, dependency, workflow permission or installer-upload setting changed. Native installer creation and install/reinstall/uninstall preservation must pass on the new committed revision in CI. The first run's successful application checks and ARM package do not substitute for that Windows result. No release is published.
