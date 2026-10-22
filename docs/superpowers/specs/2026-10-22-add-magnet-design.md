# Add torrent and magnet links

The user requested Add wording and either one combined entry or an Add/magnet
split control. Use the offered split approach: Add torrent selects local files;
the adjacent monochrome magnet icon opens a paste field. Keep the same control
in the toolbar and empty state, with keyboard focus, a tooltip and an accessible
label. Rename the file picker and tray action to Add torrent as well.

A single input chooser would add another step for local files; two separate text
buttons would take more space. The split control follows the user's alternative.

Magnet links need metadata before file selection. Use librqbit's metadata-only
lookup, then the existing preview, destination, file selection and confirmation.
Run lookup independently of the command loop, with a two-minute deadline and
Cancel. Cancellation and Quit must stay responsive. No payload/session entry is
created until confirmation. Reject malformed/non-magnet/v2-only inputs locally;
ignore optional magnet file-selection hints and keep explicit preview selection.
Limit link length and strip unsupported fields before the dependency parser.
The pinned bencode decoder is patched to cap nesting at 128 containers during
network decoding, before metadata reaches the application's existing guard.
Keep its provenance, license and a controlled-peer regression with the patch.

Keep Windows compatibility through shared Rust/Slint. Do not register system
magnet URL handling in this change; pasted links are supported. Rebuild the local
Mac app and verify both buttons and cancellation. Public-network downloads are
not needed for validation; use controlled loopback peers.
