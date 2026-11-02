[Русская версия](2026-11-02-priority-menus-ru.md)

# Priority menus implementation plan

> Execute inline using executing-plans and TDD, then one fresh final review. Preserve uncommitted work; no commits, push or publication.

**Goal:** approved icon/context-menu UI with real independent torrent and file priority.
**Architecture:** manifest v4 -> paused durable policy transaction -> portable weighted shared download admission; existing file scheduling remains. UI shares priority icons/menu entries.
**Tech:** Rust 1.92.0, locked dependencies, librqbit 9.0.1, Slint 1.18.1, jobs 2, Mac deployment 14.0.
**Spec:** [branch design](../../architecture/priority-menus-experiment.md).

## Global constraints

Main/AppKit/other experiments and payload safety unchanged. RU/EN pairs. Default Normal. No new settings, dependencies, releases or installer uploads. Preserve historical evidence and ignored recovery files.

## Review focus

Cancellation and limit changes cannot strand admission; peer count cannot multiply share; idle torrents do not retain credit. Schema 4 cannot be downgraded by file edits. Damaged/future saves cannot apply transient policy. Menu target survives virtual-row rebuild and keyboard use. Selection/busy/modal gates apply to both pointer and callback paths.

### Task 1: Durable torrent priority

- [x] RED: manifest v4, old defaults, invalid/future preservation; controller change/restart, save and engine failures.
- [x] GREEN: domain priority field, engine API, restore and shared durable paused policy change.
- [x] Verify scoped storage/controller tests.

### Task 2: Weighted download requests

- [x] RED: actual admission weights independent of peer count, cancellation/idle/limit transitions, unlimited bypass; local two-torrent transfer preference/reversal.
- [x] GREEN: portable shared weighted queue, native atomic priority set before transfer, adapter.
- [x] Verify focused scheduler and real swarm tests.

### Task 3: Slint icons and context menus

- [x] RED: rendered file/torrent right click and icon click use correct IDs, current selection/check, no eager changes, busy and narrow layout.
- [x] GREEN: shared icon/menu components, independent torrent field/callback and stable file popup outside virtual repeater. Keep existing actions and checkboxes.
- [x] Verify focused UI tests; capture native preview where available.

### Task 4: Final verification

- [x] fmt, strict Clippy, all-target tests, locked release, Mac lifecycle; independent final review and fixes with reproducing tests.
- [x] Paired evidence/handoff with platform limits. No commit/push/merge/publication.

Outcome: 195/195, fmt/Clippy/release with local workaround/Mac lifecycle/native window verified. Pixel RED→GREEN fixes glyph centering. Minor direct-callback guard hardening deferred; Windows/Intel CI not run. [Evidence](../../validation/priority-menus-2026-11-02.md).

Follow-up: deferred direct UI callback guards are [fixed and verified](../../validation/ui-transfer-guards-2026-11-02.md), 199/199.
