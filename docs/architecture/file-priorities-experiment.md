[Русская версия](file-priorities-experiment-ru.md)

# File priorities experiment — stage 2

> The approved [2 November follow-up](priority-menus-experiment.md) replaces dropdowns with icons/context menus and adds independent torrent priority. The stage-2 behavior below is historical.

The maintainer approved the full two-stage experiment and requested Low / Normal / High on `experiments/file-priorities`. Stage 1 file selection stays intact. This branch-only addendum extends the release spec without changing main or the AppKit/performance experiments.

Every non-padding file has Normal priority by default. The initial add dialog remains Normal-only; the controller rejects prefilled priority overrides rather than applying a policy absent from the manifest. A compact dropdown next to its checkbox applies changes immediately. Disabling a file keeps its saved priority; re-enabling restores it. Priority can be prepared on an excluded file without enabling it. Checking/error/busy controls remain disabled. File IDs, not visible row indices, identify changes.

Priorities are preferences for new piece reservations within one torrent: High before Normal before Low, filename order within a tier. Unavailable high-priority pieces must not block lower available pieces. Steady-state ranking does not cancel in-flight requests. Applying an edit briefly pauses this torrent; incomplete in-flight chunks may be cancelled and requested again, while verified pieces are reused; shared boundary pieces follow their highest-priority selected owner. This is not a hard bandwidth allocation or a queue between torrents.

Use the exact librqbit 9.0.1 package as a documented local vendor patch: expose a validated complete file order, initialize it before any transfer, retain it across pause/resume and read it in the existing piece scheduler. Existing default scheduling and capability storage stay unchanged. No dependency version upgrade. Application levels remain outside librqbit. A wrapper that repeatedly toggles selection would change completion semantics and exclude useful data, so is rejected; upgrading the engine is outside this experiment.

Persist sparse non-Normal overrides by FileId in manifest v3; v1/v2 load as Normal. Reject unknown priority values, duplicate overrides, padding/unknown file IDs and future schemas without overwriting. File selection edits must never downgrade v3. Restore configures priority while paused before Checking can resume transfers. Retain the same durable intent/error reconciliation/retry guarantees as stage 1. Completion notifications stay at most once.

Tests must demonstrate actual High-first reservation/download over a controlled swarm, Low still completes afterward, equal-tier/default order, changes while paused/active, restore, excluded-file retention, invalid inputs, failures after saving, and UI callback identity/rollback. Windows/Intel source compatibility is required; local Mac execution is not CI or physical evidence.
