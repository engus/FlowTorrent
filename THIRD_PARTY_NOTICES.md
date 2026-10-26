# Third-party notices

FlowTorrent's own code and documentation retain the author's [MIT license](LICENSE).
Dependencies retain their own licenses; the complete application is not described
as an entirely MIT-licensed dependency chain.

Target-specific test packages include `ThirdParty/THIRD_PARTY_NOTICES.html`, raw
supplemental license/NOTICE files, Rust standard-library copyright/license files,
unchanged MPL component source and `NOTICE_MANIFEST.json`. On Mac these are in
`FlowTorrent.app/Contents/Resources`; on Windows they are in the install folder.
The manifest binds notices to the exact Cargo.lock, target and policy. Windows
packages also include the Inno Setup license.

Generate these files with the pinned `cargo-about 0.9.2` wrapper documented in
[test packaging](docs/test-packages.md). Missing crate coverage, modified Slint
license text or stale manifests fail packaging. Final review of generated notices
and native runtime obligations remains part of release approval.

Slint 1.18.1 is used under its bundled Royalty-free License 2.0. Preserve the
visible official badge on the public project/download pages. librqbit and the
other dependencies keep the exact terms recorded in their source packages and
generated notices. Vendored modifications remain documented in their `PATCHES.md`
files; no application dependency upgrade is part of this packaging work.
