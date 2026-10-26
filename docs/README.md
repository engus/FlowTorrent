[Русский](README-ru.md) · [Project overview](../README.md)

# FlowTorrent documentation

The current app is a shared Rust/Slint Windows and macOS client, package version 0.1.0 in development. Product descriptions below follow the approved current UI; dated reports retain their original scope and evidence.

## Using FlowTorrent

- [User guide](user-guide.md) — installation model, adding, transfers, settings, background operation and safe removal.
- [Localization](localization.md) — 14 languages, system fallback, catalog workflow and acceptance limits.
- [Original brandbooks](brandbook/README.md) — English and Russian PDF originals, provenance and checksums.
- [Repository About](repository-about.md) — description, topics, homepage and author/support boundaries.

## Development and design

Read [the 1.0 specification](spec-1.0.md), [development handoff](development-handoff.md) and [ordered implementation plan](superpowers/plans/2026-10-21-flowtorrent-1.0.md) first. These older baseline documents are currently in Russian. Approved subsequent decisions supersede their earlier exclusions only within the recorded scope:

- [Slint redesign](superpowers/specs/2026-10-26-slint-redesign.md) — shared UI, filtering, settings, safe Trash and login integration.
- [Bottom drawer](superpowers/specs/2026-10-26-slint-drawer.md) — collapsible sidebar, details and autosaved settings.
- [Localization and repository plan](superpowers/plans/2026-10-26-localization-and-repository.md) — approved language set and repository presentation.
- [Menu-bar/tray preference](superpowers/plans/2026-10-26-tray-preference.md) — approved default-on toggle and independent Mac status-item lifetime.
- [Latest Slint handoff](slint-redesign-handoff.md), [design audit](slint-design-audit-2026-10-26.md), [root code audit](../claude-audit.md).
- [Local Mac builds](local-macos.md), [dependency policy](dependency-policy.md), [third-party notices](../THIRD_PARTY_NOTICES.md), [Lucide assets](../assets/icons/lucide/README.md).

The root [agent guidance](../AGENTS.md) defines storage, persistence, threading and verification invariants. New feature requests still require an explicit scope decision.

## Verification and release evidence

- [Release checklist](release-checklist.md) and [engineering validation](engineering-validation.md) — release gates and recorded evidence.
- [Current drawer validation](slint-drawer-validation.md), [Slint parity validation](slint-parity-validation.md), [redesign validation](slint-redesign-validation.md) — native UI checks and their limits.
- [Windows validation, 23 October](windows-validation-2026-10-23.md) and [Mac validation, 23 October](mac-validation-2026-10-23.md) — dated platform observations.
- [Test packaging](test-packages.md), [22 October readiness](release-readiness-2026-10-22.md), [release notes](../CHANGELOG.md).
- [Resource measurements](performance.md) and [memory optimization](memory-optimization.md) — workloads, results and historical raw data.

CI, local probes and physical checks provide different evidence. Preserve the revision, platform and conditions attached to each report. Physical Windows/DPI, supported Mac/Intel coverage, overnight and other open checks are not closed by documentation, a package build or a CI result. Public signing/notarization remains pending; publishing releases, tags and installers is a separate maintainer action.
