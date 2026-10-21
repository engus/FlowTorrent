use std::{net::SocketAddr, path::Path, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use librqbit::{AddTorrent, AddTorrentOptions, ListenerOptions, Session, SessionOptions};
use sha1::{Digest, Sha1};

pub fn payload() -> Vec<u8> {
    (0..512 * 1024)
        .map(|n| ((n * 37 + 11) % 251) as u8)
        .collect()
}

pub fn torrent_bytes(bytes: &[u8], private: bool) -> Vec<u8> {
    let pieces: Vec<u8> = bytes
        .chunks(16_384)
        .flat_map(|p| Sha1::digest(p).to_vec())
        .collect();
    let mut result = format!(
        "d4:infod6:lengthi{}e4:name11:fixture.bin12:piece lengthi16384e6:pieces{}:",
        bytes.len(),
        pieces.len()
    )
    .into_bytes();
    result.extend(pieces);
    if private {
        result.extend(b"7:privatei1e");
    }
    result.extend(b"ee");
    result
}

pub fn multi_torrent_bytes(first: &[u8], second: &[u8]) -> Vec<u8> {
    let joined: Vec<u8> = first.iter().chain(second).copied().collect();
    let pieces: Vec<u8> = joined
        .chunks(16_384)
        .flat_map(|p| Sha1::digest(p).to_vec())
        .collect();
    let mut result = format!("d4:infod5:filesld6:lengthi{}e4:pathl9:first.bineed6:lengthi{}e4:pathl10:second.bineee4:name6:bundle12:piece lengthi16384e6:pieces{}:", first.len(), second.len(), pieces.len()).into_bytes();
    result.extend(pieces);
    result.extend(b"ee");
    result
}

pub fn local_options() -> SessionOptions {
    SessionOptions {
        dht: None,
        disable_trackers: true,
        disable_local_service_discovery: true,
        persistence: None,
        listen: Some(ListenerOptions {
            listen_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub async fn seed(
    folder: &Path,
    metadata: &[u8],
    bytes: &[u8],
) -> Result<(Arc<Session>, SocketAddr)> {
    std::fs::write(folder.join("fixture.bin"), bytes)?;
    let session = Session::new_with_opts(folder.to_owned(), local_options()).await?;
    let handle = session
        .add_torrent(
            AddTorrent::from_bytes(metadata.to_vec()),
            Some(AddTorrentOptions {
                overwrite: true,
                ..Default::default()
            }),
        )
        .await?
        .into_handle()
        .context("seeder handle")?;
    tokio::time::timeout(Duration::from_secs(15), handle.wait_until_completed()).await??;
    let addr = session.listen_addr().context("seeder listener")?;
    Ok((session, addr))
}
