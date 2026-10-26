# Local Mac application milestone

Approved scope: the user asked to continue the existing code as a local Mac
`.app` they can run and test here, keeping Windows compatibility. This narrows
task 10 of the existing 1.0 plan to local Mac packaging.

Keep Rust, librqbit, Slint and the existing `com.engus.flowtorrent` identity.
Build the native architecture by default; allow the other existing Mac target
explicitly. Package the existing executable, icon and document declarations in
a conventional app bundle, signed ad hoc for local use. Do not force default
file handlers. Do not add a Swift UI, engine features, Developer ID credentials,
notarization, a DMG or a public release in this milestone.

Approaches: a small native-tools packaging script fits the existing plan and
requires no new Cargo dependency. A general bundling framework adds dependency
and configuration overhead; a separate Xcode application duplicates lifecycle
work that is already implemented. Use the script approach.

The packaging command applies the proven host-only stripping override on Mac;
Windows Cargo configuration remains unchanged. Python 3.9 standard library
handles bundle assembly and validation; macOS `sips`, `iconutil` and `codesign`
provide image conversion and local signing. Version comes from Cargo metadata.
Existing app replacement requires an explicit flag and a matching bundle ID.
Stage and sign a new bundle before replacing a previous one.

Acceptance: runnable native `.app`, correct version/identifier/icon/minimum
macOS 14, Finder `.torrent` opening without a forced default, working
close/reopen/Quit, repeatable command, packaging behavior tests, and the existing
Rust tests. Record actual runtime checks and remaining manual checks separately.

Shared reliability defects from the audit remain subsequent work; packaging
does not claim to resolve them. Windows CI must retain its existing checks.
