[Русская версия](priority-menus-2026-11-02-ru.md)

# Priority icons and context menus — 2 November 2026

Status: implemented, independently reviewed and locally verified on Mac ARM. Windows/Mac Intel CI remain open. No commit, push, merge or publication.

Worktree `experiments/file-priorities`, base `142f2bb75a4c4c8769f15a44d0f4fface33406b0` plus the preserved uncommitted file-selection/priority stages and this change. [Design](../architecture/priority-menus-experiment.md) · [Plan](../superpowers/plans/2026-11-02-priority-menus.md) · [Historical stage 2](file-priorities-stage2-2026-11-01.md).

## Result

Lucide equal / chevrons-up / chevrons-down icons show independent Normal / High / Low priorities on torrent and file rows. Files keep their inclusion checkboxes and saved priorities when excluded. A click on the icon or a right-click on the row opens the menu. Mac Control-click is supported. Menus show the current value, support keyboard selection, and preserve existing torrent actions. Normal is the default. Popup targets are kept outside virtual rows; opening a menu does not change priority. No dropdown remains in the file list.

File scheduling is unchanged from stage 2. Torrent priority is a relative preference at the shared download cap: Low 1, Normal 2, High 4. The portable vendored queue uses one virtual clock per torrent, regardless of peer count, and no scheduler task. Cancelled/idle flows retain no credit. Changing the limit wakes waiters; unlimited download introduces no artificial delay. This is not an exact throughput promise, an upload preference or a per-torrent hard cap. Existing protocol/storage/privacy and upload handling remain intact.

Torrent priority is saved in manifest v4. Schemas 1–3 mean Normal and cannot contain a non-Normal torrent priority. Invalid enum values, damaged/future files and invalid ownership still fail without overwrite. File edits cannot downgrade v4. Applying an edit reuses the durable paused policy transaction; failures after replacement are reconciled, stale resumes are blocked and Retry/restore configures the saved policy before transfer. Initial additions remain Normal-only. No new setting or dependency version is introduced.

## Evidence

Mac ARM, macOS 27.2 (26B123), Rust 1.92.0, locked dependencies, jobs 2, deployment target 14.0. Controlled private loopback swarms only. Raw logs, initial recovery archive, incremental review patch and frozen source hashes are under ignored `_work/validation/priority-menus-20261102/`.

- Initial all-target run: **194 passed, 0 failed, 0 ignored**, 44 harness summaries.
- Storage RED: schema 4 rejected; then GREEN roundtrip, old defaults, invalid values and downgrade rejection.
- Controller RED: missing command/field; GREEN durable independent priority, restart, selection/schema retention, save errors before/after atomic replacement and injected engine failure/Retry.
- Scheduler GREEN: High preference with more Low peers, cancellation and unlimiting release waiters. Real two-torrent transfers demonstrate preference reversal and complete byte equality for both payloads. An independently run unweighted mutation failed the throughput preference assertion; original implementation was restored.
- Rendered Slint GREEN: opening does not mutate priority, current-value keyboard navigation dispatches actual IDs, disabled controls and failed-send checkbox rollback, wide/narrow layout. Existing context-menu actions and double-click tests still pass.
- Final all-target run: **195 passed, 0 failed, 0 ignored**, 44 harnesses; fmt, vendor formatting and strict all-target Clippy pass. An equivalent `match` rewrite was needed for the included vendored source's Clippy style check.
- Standard release repeated the known local `zerofrom_derive` load failure (E0463/E0432). Final release with local `--config 'profile.release.build-override.strip="none"'` passed in 2m 03s; repository Cargo profiles were not changed. Mac lifecycle passed. Main executable SHA256: `f114a47a4077efc99127201396c125a6530371e046990fd93f665b2007979b35`.
- Native light Retina check of final `FlowTorrent Priority Menus Updated`: icon and torrent right-click open the menu; Low persists in v4 while Paused, then returns to Normal. Right-click on `02-high.bin` saves High for FileId 1 independently of the Normal torrent; its icon menu reflects High and restores Normal. JSON was checked independently of the UI. All three files remain selected with Normal, the torrent is stopped and the drawer open. Ordinary sessions and the previous test window's open file chooser were left untouched.
- Native inspection caught a torrent priority glyph 9 logical px above the row center. A vertical centering container fixes it. A pixel test compares the actual SVG centers: unwrapped RED 183 versus 192, wrapped GREEN. Earlier pointer-hit/incorrect-fallback-menu-coordinate attempts are not evidence; their raw logs are retained. Pixel tests explicitly use the light palette for CI stability. Final native alignment was also inspected.
- All 373 frozen inputs matched after verification; Cargo.lock was unchanged during this task. The native preview is only ad-hoc signed, not a signed release/installer. Preview executable SHA256: `c1dfd51be5653579b6a65fdb0bf7b2a245e7b1fe46df920eeb1a7c31b37d8fcd`.

## Review and limits

One fresh read-only review found no Critical/Important correctness issue. Its vendor-documentation note was addressed as planned. The review recorded a Minor callback-hardening gap: existing file callbacks rely on pending/confirmation and disabled pointer controls rather than explicitly checking every `busy`/`stopping` flag; file and torrent priority callbacks do not explicitly reject About. Direct programmatic callback invocations can differ from the protected pointer path. No normal pointer-driven data loss was demonstrated; the shared callback guard and direct-callback tests are now verified in the [follow-up fix](ui-transfer-guards-2026-11-02.md).

Review rulings: no-cap relative throughput and upload priority are outside the stated behavior; runtime/resource conclusions require evidence beyond source review. No resource benchmark or claims about CPU/RAM were made. Windows x64 and Mac Intel CI and physical checks were not run for this uncommitted change; portable source is not proof of those executions. Main, AppKit, other experiments and ordinary user sessions were not modified. No releases, tags or installers were published.
