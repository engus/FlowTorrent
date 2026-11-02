[Русская версия](file-selection-experiment-ru.md)

# File selection experiment

Approved on 2026-10-31 for `experiments/file-priorities` only. This is an explicit experimental extension of spec 1.0; it does not expand the release scope on main.

Stage 1 adds per-file checkboxes to the single-torrent details pane. Changes apply immediately, persist in the app manifest before acknowledgment, retain existing data, and preserve the torrent's desired active/paused state. The actor composes individual toggles against its latest manifest, not a stale UI list. Checking, error recovery and interrupted Trash operations block edits.

An empty selection is valid after adding: display “No files selected”, zero selected-byte progress and no completion notification. New adds still require at least one file. Selection survives restore, which remains paused until checking completes. Manifest version 2 allows empty selection; version 1 is read unchanged and upgraded only on a selection change. Older clients must reject version 2 rather than overwrite it.

librqbit 9.0.1 supports `Session::update_only_files`. Apply it to a paused handle, then resume only if the persisted desired state is active. On persistence failure retain the old policy. On engine failure after persistence keep the torrent paused in an explainable error; Retry restores the durable policy. Never resume a stale policy. Completion remains at most once per torrent; do not reset its notification flag.

All payload access retains the existing pinned handles and path safety. Disabling does not delete/truncate data; boundary pieces may contain bytes belonging to a disabled file. Trash acts on owned payload files, including files disabled after downloading, retaining all existing identity/hard-link checks. Remove from list keeps payload.

Stage 2 adds real Low/Normal/High scheduling, persistence and tests through a separately reviewed librqbit patch. It is not implemented by stage 1. Windows and Mac source compatibility are required; local Mac verification is not Windows/Intel runtime evidence. AppKit prototype, other experiments and main stay unchanged.

Stage 2 is now implemented separately: [priority behavior](file-priorities-experiment.md), [local verification](../validation/file-priorities-stage2-2026-11-01.md). Priority edits upgrade to v3; stage-1 selection edits preserve that version.
