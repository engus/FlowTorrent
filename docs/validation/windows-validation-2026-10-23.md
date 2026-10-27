# Native CI validation — 23 October 2026

The prepared branch was pushed with the maintainer's authorization. Test-package
uploads remain disabled; no release or tag was created. The application retains
version 0.1.0 and the Windows installer is unsigned.

## Confirmed baseline

[Check 39837098556](https://github.com/engus/FlowTorrent/actions/runs/39837098556)
tested `18e929e4ccab5fa9c1c5d786e3010efdc0305d27` on native Windows x64, Mac ARM64
and Mac Intel runners. All three core jobs and dependency policy passed.
The full workflow concluded **success**, with all eight jobs successful.

| Target | Rust all-target records | Bencode records | Python packaging suite |
| --- | ---: | ---: | --- |
| Windows x64 | 111 | 18 | 40 discovered; two Mac-only tests skipped |
| Mac ARM64 | 112 | 18 | 40 passed |
| Mac Intel | 112 | 18 | 40 passed |

Rust records had zero failures and zero ignored tests. Formatting, check and
Clippy passed on all three targets. Mac lifecycle probes passed. The Windows
version-resource regression passed under both Windows PowerShell 5.1 and
PowerShell 7, including rejection of wrong numeric versions, wrong identity,
prerelease text and leading whitespace.

Both native Mac package jobs built the app and DMG, verified the ad-hoc
signature and native binary metadata, and passed `hdiutil verify`.
The Windows package job compiled the optimized AMD64 application and
the actual Inno Setup installer; numeric and textual resource checks passed for
both. Native package job IDs are `120032698112` (ARM64), `120032698005` (Intel)
and `120032698142` (Windows).

Windows job `120032698142` passed the disposable installation test, including
cleanup, on Windows Server 2022 (`10.0.20348.0`). Its printed evidence confirms
per-user install, same-version reinstall, the quoted Open With command,
unchanged default handler, shortcut AppUserModelID, unchanged state/payload
hashes and preservation of an unowned file during uninstall.

[The preceding run](https://github.com/engus/FlowTorrent/actions/runs/39832433850)'s
cleanup attempted to delete a registry value through a
read-only handle. That `finally` exception could conceal an earlier assertion,
so that failed run was not counted as full preservation evidence. The successful
rerun opens an explicit writable handle, disposes it and prints the evidence
only after cleanup succeeds.

All three package-upload steps were **skipped**. The run's artifact API lists
only `dependency-policy-inventory` (344,729 bytes), with no installers or DMGs.
Generated packages stay outside the Cargo cache. The later documentation-only
commit does not change the validated application, packaging scripts or workflows;
CI evidence above is bound to source commit `18e929e`.

## Packaging issues resolved during native validation

- Put the Mach-O input before `lipo -verify_arch`, matching the parser in the
  runner's older Apple toolchain. Native ARM64 and Intel packaging then passed.
- Locate a licensed Inno Setup installation before compiling the application,
  checking the PATH candidate and standard installation directories. The native
  runner selected `C:\Program Files (x86)\Inno Setup 6` and compiled setup.
- Set Inno's textual product version explicitly from Cargo, while keeping both
  numeric versions four-part. Accept Inno's fixed-width trailing padding during
  verification; preserve strict numeric, identity and version comparisons.
- Keep installers and notices outside the Rust build cache using
  `target/ci-cargo` for Cargo and `target/test-packages` for package output.
- Restore the disposable installer's test association through a writable
  registry handle. The full native test, including cleanup, then passed.

## Scope and remaining release gates

These changes affect packaging, tests and CI. Rust application source, Slint UI,
Cargo dependencies and vendored engine code are unchanged from production base
`91872e0`. The AppKit prototype remains in its separate worktree; its delivered
executable SHA-256 is unchanged:
`68d8a0c9aeb05b5f0659e4bc0159bff6dc1b5df0d8aa8795e242e7ff0798d74a`.
The original checkout's uncommitted documentation was preserved.

The disposable Windows test uses raw state/settings sentinels and payload hashes.
It does not launch the application, establish valid-session cross-version
restoration, or replace physical Windows 10/11 testing. Installed notification
delivery/denial, native DPI/dialogs, clean supported-system installation, real
sleep/wake, overnight operation, signing/notarization and final redistribution
notice review remain open in [the release checklist](release-checklist.md).

**1.0 is not release-ready.**
