[Русский](slint-visible-cpu-2026-10-28-ru.md)

# Visible-window CPU investigation — 28 October 2026

## Scope and method

The maintainer requested an explanation for the visible Slint window's 9–17% short-interval CPU readings without interrupting the overnight run. Source is `142f2bb75a4c4c8769f15a44d0f4fface33406b0`, unchanged desktop library, release software renderer, macOS 27.2 / 26B123, Apple Silicon, actual scale 2. The owned diagnostic client PID was 10245; the source, receiver, driver and caffeinate remained running. No window, torrent, rate, binary, production setting or dependency was changed.

Two read-only `/usr/bin/sample` captures sampled the same visible client for ten seconds each at five-millisecond intervals. Raw reports and profile timestamps are preserved under ignored `_work/validation/overnight-20261027/cpu-investigation/`. `profile-window.json` marks the broad profiling interval: exclude it from unprofiled CPU comparisons, because profiling adds overhead. No debugger, process restart, new test swarm or renderer switch was used.

## Findings

Both profiles show the hot UI path through `WinitWindowAdapter::draw`, `WinitSoftwareRenderer::render`, CoreAnimation transaction commit, `CA::Render::copy_image` and CoreGraphics/vImage colour conversion. Slint scene traversal, text and SVG rasterization are also present. Snapshot processing is sampled infrequently; this evidence does not support treating the torrent controller's formatting work as the dominant cost.

| Inclusive stack occurrences | Capture 1 | Capture 2 |
| --- | ---: | ---: |
| Winit window draw | 141 | 157 |
| CoreAnimation copy_image | 103 | 110 |
| Slint SVG rasterization | 9 | 13 |
| UiState::apply | 0 | 3 |
| UiState::refresh_view | 0 | 2 |

These are inclusive sampled stack counts, not CPU percentages or frame counts. Absence from a short sample does not prove a function costs nothing. The earlier matched hidden five-download windows averaged approximately 0.67% and 0.66% of one logical core; their footprint medians were 54.72 and 54.92 MiB. Visible and hidden phases, window-size changes and profiling must remain separate in the overnight report.

Inspection of the exact locked backend sources explains why presentation is expensive:

- `softbuffer 0.4.8`, `src/backends/cg.rs`, `buffer_mut`: allocates a new zeroed `width * height` pixel vector for each buffer.
- Its `BufferInterface::age()` returns zero. Slint 1.18.1 `renderer/sw.rs` selects `RepaintBufferType::NewBuffer` for this age, preventing reuse-based partial repaint of that buffer.
- `present_with_damage(self, _damage)` calls `present()` and ignores damage rectangles. Presentation creates a full CGImage and updates CALayer contents. The sampled colour conversion/copy path occurs at this boundary.
- At scale 2 there are four physical pixels per logical pixel; a 32-bit full-frame buffer therefore grows with physical window area. This also explains additional transient memory pressure, but does not by itself prove that all visible-memory growth is harmless or rule out a leak.

The controller publishes snapshots every 500 ms. While visible, the UI applies them and refreshes display models. Existing row updates already compare old/new rows and avoid replacement when unchanged. Slint properties also compare values before marking dependent properties dirty. Therefore a blanket claim that every equal property assignment forces a redraw would be incorrect. Changing rates/progress and other display changes can still schedule expensive frames. The exact number of frames, dirty regions and the contribution of each display field were not instrumented in this live run.

## Next experiments, not implemented

1. In a separate controlled candidate, coalesce visible statistics presentation and update only display changes that matter; preserve immediate commands, errors and state transitions. Compare CPU with the same window size/workload and measure responsiveness.
2. Measure frame counts, changed regions and SVG-cache behaviour before changing icon assets. SVG work is secondary in these captures, not the main demonstrated bottleneck.
3. If presentation still dominates, compare a supported GPU renderer on Mac with the current software renderer, including memory and package-size costs. Keep the pinned versions, Windows behaviour and isolated AppKit prototype unchanged unless separately authorized.

No performance fix or CPU reduction is claimed. The live overnight test continues, and it is not yet a passed eight-hour run. No commit, push, tag, installer upload or release was made.
