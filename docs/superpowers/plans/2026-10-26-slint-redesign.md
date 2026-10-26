[Русский](2026-10-26-slint-redesign-ru.md)
# Slint redesign implementation plan

**Goal:** Transfer the approved v42 design into the production Slint client and connect real engine data.
**Architecture:** Existing AppController/Tokio engine, bounded commands, coalesced watch snapshots, pure ID-based view state, native Slint components.
**Stack:** Rust 1.92, Slint 1.18.1, librqbit 9.0.1, cap-std; locked dependencies.
**Spec:** ../specs/2026-10-26-slint-redesign.md
**Global constraints:** Existing slint-release-readiness worktree; AppKit untouched; -j 2; MACOSX_DEPLOYMENT_TARGET=14.0; all Cargo --locked. Preserve path safety, durable intent, corrupt data, notifications and badges. Never publish.
**Review focus:** Selection scope, hidden update cost, per-file truth, old settings compatibility, Trash ownership/crash recovery, OS integration rollback.

## Task 1: Display state and real engine details
Files: src/domain.rs, src/engine/{mod,librqbit}.rs, src/app/{commands,controller}.rs, src/ui/{models,view_state}.rs, tests/ui_models.rs, tests/view_state.rs.
Interfaces: stable IDs, filter/sort/selection state; selected detail snapshot; uploaded bytes; bulk pause/resume command.
Write failing tests for ordinary versus modifier selection, reconciliation, completed filtering, ETA/ratio math and global bulk scope; implement. Fetch details only for one inspected visible torrent. Verify focused tests. Commit feat: add native redesign view state and transfer details.

## Task 2: Brand and Slint main layout
Files: assets/brand, ui/{main,theme,models,details}.slint, src/ui/controller.rs.
Interfaces: Task 1 models/callbacks; preserve existing add/settings bindings. Copy only required brand assets and license. Implement sidebar, global labeled actions, search, row ellipsis, directional progress, real rates, footer and pinned right details. Compile and use isolated ui_smoke. Commit feat: transfer branded torrent layout to Slint.

## Task 3: Settings and native login
Files: src/{settings,rate}.rs, src/platform/login.rs, ui/settings.slint, src/ui/controller.rs, tests/settings.rs.
Interfaces: compatible serde defaults, explicit Save/Cancel, exact Mbit conversion; injectable login provider. Write failing rate/migration/cancel and ownership tests. Implement theme/language, rate presets, folder/notification and opt-in login with rollback. Verify tests and UI. Commit feat: connect redesigned preferences and login integration.

## Task 4: Safe Trash operation and menus
Files: src/app/{targets,trash,commands,controller}.rs, src/platform/trash.rs, tests/trash.rs, UI action/confirmation code.
Interfaces: explicit immutable IDs, OS Trash provider, private staging/journal, retained records on failure. Write failing refusal/replacement/link/crash/failure tests before implementation. Keep storage deletion disabled. Verify local controlled fixtures and UI confirmation. Commit feat: move owned torrent payload to system Trash safely.

## Task 5: Native validation and handoff
Run fmt, Clippy all targets -D warnings, tests all targets, release build and Mac lifecycle probe. Build an unsigned test .app. Exercise isolated real-engine add/pause/resume/details/settings/search and inspect native rendering. Fresh whole-branch review; fix important findings with reproducing tests. Update paired validation/handoff documents with evidence and honest remaining Windows physical/CI checks. Commit docs: record native redesign validation. Keep branch local, do not publish.
