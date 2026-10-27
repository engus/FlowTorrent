#[allow(dead_code)]
mod support;

use flowtorrent::engine::{TorrentEngine, librqbit::LibrqbitEngine};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
};

struct CountedSystem;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
fn allocated(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::SeqCst) + bytes;
    PEAK.fetch_max(live, Ordering::SeqCst);
}
// SAFETY: Every request is delegated unchanged to System. Counters use only
// atomics and never allocate, retaining the underlying allocator's contracts.
unsafe impl GlobalAlloc for CountedSystem {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: layout is supplied by the allocator caller.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
        // SAFETY: pointer/layout come from this allocator's matching allocation.
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: Caller supplies a live allocation and valid new size.
        let replacement = unsafe { System.realloc(pointer, layout, new_size) };
        if !replacement.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
            allocated(new_size);
        }
        replacement
    }
}
#[global_allocator]
static ALLOCATOR: CountedSystem = CountedSystem;

// One test in its own binary: unrelated test allocations must not share this
// peak. The threshold has several MiB of slack for runtime bookkeeping.
#[tokio::test]
async fn metadata_and_preview_selection_avoid_large_duplicate_allocations() -> anyhow::Result<()> {
    let root = support::tempdir()?;
    let input = root.path().join("extension.torrent");
    let original = support::torrent_bytes(&support::payload(), false);
    let expected = {
        let engine =
            LibrqbitEngine::with_options(root.path().to_owned(), support::local_options(), vec![])
                .await?;
        let plain = root.path().join("plain.torrent");
        std::fs::write(&plain, &original)?;
        engine.inspect(&plain).await?.id
    };
    let mut metadata = original;
    assert_eq!(metadata.pop(), Some(b'e'));
    metadata.extend_from_slice(b"16:x-memory-paddingl");
    for _ in 0..300_000 {
        metadata.extend_from_slice(b"i0e");
    }
    metadata.extend_from_slice(b"ee");
    std::fs::write(&input, &metadata)?;
    let input_bytes = metadata.len();
    drop(metadata);
    let engine =
        LibrqbitEngine::with_options(root.path().to_owned(), support::local_options(), vec![])
            .await?;
    let base = LIVE.load(Ordering::SeqCst);
    PEAK.store(base, Ordering::SeqCst);
    let preview = engine.inspect(&input).await?;
    let peak = PEAK.load(Ordering::SeqCst).saturating_sub(base);
    println!("metadata_bytes={input_bytes} peak_additional_rust_bytes={peak}");
    assert_eq!(preview.id, expected);
    assert_eq!(preview.metadata, std::fs::read(&input)?);
    let prefix = "p".repeat(200);
    let files = (0..10_000)
        .map(|index| flowtorrent::domain::TorrentFile {
            id: flowtorrent::domain::FileId(index * 2),
            relative_path: format!("{prefix}/file-{index:05}.bin").into(),
            length: 1,
            is_padding: index % 10 == 0,
        })
        .collect();
    let large_preview =
        std::sync::Arc::new(flowtorrent::domain::TorrentPreview { files, ..preview });
    let base = LIVE.load(Ordering::SeqCst);
    PEAK.store(base, Ordering::SeqCst);
    let mut selection = flowtorrent::ui::models::PreviewSelection::new(&large_preview);
    let selection_peak = PEAK.load(Ordering::SeqCst).saturating_sub(base);
    println!("file_count=10000 peak_additional_selection_bytes={selection_peak}");
    assert_eq!(selection.selected_bytes(), 9_000);
    selection.set_checked(flowtorrent::domain::FileId(2), false);
    selection.set_checked(flowtorrent::domain::FileId(0), true);
    assert_eq!(selection.selected_bytes(), 8_999);
    assert_eq!(selection.selected_files().len(), 8_999);
    drop(selection);
    assert_eq!(std::sync::Arc::strong_count(&large_preview), 1);
    assert!(
        peak < 8 * 1024 * 1024 && selection_peak < 512 * 1024,
        "unnecessary allocations: format={peak} bytes for {input_bytes} input bytes; selection={selection_peak} bytes"
    );
    Ok(())
}
