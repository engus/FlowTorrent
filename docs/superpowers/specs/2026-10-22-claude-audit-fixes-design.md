# Claude audit fixes

Validate the ten static findings against the current local Mac/magnet branch,
then repair confirmed defects without changing pinned engine/UI versions,
payload deletion policy or the four-setting scope. Preserve the main checkout's
in-progress translations and retain the existing app packaging.

1. Distinguish startup, runtime and shutdown errors; capture early failures in
   bounded local diagnostics with stderr fallback.
2. Settings corruption must not lock transfers: recover defaults (or a separately
   saved valid recovery file), show the preserved original path and next step,
   and save corrected settings separately. A corrupt session remains read-only.
3. Retain uncommitted claimed handles across cancel/folder changes. Reuse only
   this process's unchanged empty files; never delete or overwrite payloads.
4. Share immutable previews with Arc across controller/watch/UI ticks.
5. Remove obsolete owned metainfo after successful removal. Failed persistence
   or engine operations must retain a visible, recoverable registry entry.
6. Add dependency/advisory/license checks and update monitoring without hiding
   existing violations or upgrading pinned libraries without a separate review.
7. Expand compressed async blocks, share row geometry with Slint, and use a
   deterministic IPC identifier under a protected macOS per-user temp parent.

Use regression tests for each behavioral defect, controlled local swarms only,
then the full Rust/decoder/Python checks, native lifecycle probe, independent
review and a rebuilt local app. Document unresolved findings accurately.
