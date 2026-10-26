[Русский](slint-redesign-handoff-ru.md)
# Slint redesign handoff

Continue in codex/slint-release-readiness. The shared native Slint design is connected to the engine. AppKit remains isolated. The current candidate is target/local-macos/slint-parity-20261026/FlowTorrent.app, built from 53a41e8; the installed app was not replaced. The branch has been pushed for CI; no release, tag or installer upload was made. See the [layout parity and full CI follow-up](slint-parity-validation.md) for current evidence.

Read [validation and limits](slint-redesign-validation.md), the [approved transfer spec](superpowers/specs/2026-10-26-slint-redesign.md) and the [implementation plan](superpowers/plans/2026-10-26-slint-redesign.md). Existing 1.0 safety/persistence invariants still apply. The explicit new Trash decision overrides the old UI-only no-payload-removal scope; SecureStorage still cannot delete files.

The remaining physical gates include Windows DPI, keyboard, Recycle Bin and login checks, plus physical Intel execution. Full CI results are tracked in the follow-up above. Launch-at-login writes a dedicated per-user entry only after Save; tests use an injected provider. System appearance uses Winit events across both platforms. Upload percentage is explicitly session-scoped. Recovery folder relocation and lifetime statistics are not implemented.

Do not repeat the browser mockup audit or rewrite AppKit. Preserve the normal user's paused NAS torrent. Use examples/ui_smoke with a fresh absolute owned fixture directory for real-engine UI work. Keep Cargo --locked, -j 2 and macOS deployment target 14.0. Publishing remains a separately authorized action.
