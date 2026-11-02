[Русская версия](priority-menus-experiment-ru.md)

# Priority icons and menus

The maintainer approved the interactive icon/context-menu mockup on 2 November 2026 and requested implementation for torrents and files in `experiments/file-priorities`. This extends the branch-only file-priority experiment. Main, AppKit and other experiments remain untouched.

Normal is the default. Equal / double-chevron-up / double-chevron-down Lucide icons represent Normal / High / Low. A labeled icon button and right-click (Mac Control-click included) open the same menu. Keep existing torrent actions, file inclusion checkboxes, actual stable IDs, busy/checking/error gates and keyboard access. Excluded files retain their priority. Priority choices show the current value and apply immediately after durable persistence; menus must never silently change selection or priority on focus.

File priority keeps the stage-2 piece-order behavior within one torrent. Torrent priority is independent: relative download request weights Low 1, Normal 2, High 4 across competing torrents under the global download limit. It is a preference, not a promised transfer speed, sequential queue, upload priority or per-torrent hard cap. Unlimited download adds no artificial throttling. Availability and peers still determine actual throughput. Cancellation, pause/removal and idle torrents must not strand a scheduler turn. All contenders must make progress; bandwidth cannot depend on a torrent's peer count. No new setting is introduced.

Use a small portable extension to the existing exact-version librqbit patch for weighted admission at the shared download limiter. Keep upload, path/storage safety, privacy and the platform integrations unchanged. Persist torrent priority in manifest v4, with schemas 1–3 defaulting to Normal; future/damaged state stays protected. Restore configures priorities before resuming, and errors use the existing durable policy reconciliation/retry path. Initial adds remain Normal. UI and tests use actual IDs rather than row indices.
