//! Diagnostic for rate limits smaller than BitTorrent's 16 KiB request block.
use std::num::NonZeroU32;

fn main() -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let limiter = librqbit::limits::Limits::new(librqbit::limits::LimitsConfig {
            download_bps: NonZeroU32::new(1_000),
            upload_bps: None,
        });
        println!(
            "1,000 B/s, 16,384-byte block: {:?}",
            limiter
                .prepare_for_download(NonZeroU32::new(16_384).unwrap())
                .await
        );
    });
    Ok(())
}
