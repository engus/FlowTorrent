#[path = "support/lifecycle.rs"]
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod support;

use flowtorrent::{
    engine::{TorrentEngine, librqbit::LibrqbitEngine, magnet::normalize_magnet},
    error::AppError,
};
use std::time::Duration;

async fn start(
    root: &std::path::Path,
    peers: Vec<std::net::SocketAddr>,
) -> anyhow::Result<flowtorrent::app::AppHandle> {
    use flowtorrent::{
        app::{AppController, storage::SessionStore},
        settings::SettingsStore,
    };
    use std::sync::Arc;
    let paths = lifecycle::paths(root);
    std::fs::create_dir_all(&paths.download_dir)?;
    let engine =
        LibrqbitEngine::with_options(paths.download_dir.clone(), support::local_options(), peers)
            .await?;
    Ok(AppController::spawn(
        Box::new(engine),
        Arc::new(SessionStore::new(paths.clone())),
        Arc::new(SettingsStore::new(
            paths.config_file.clone(),
            paths.download_dir.clone(),
        )),
        paths,
    ))
}

async fn within(
    handle: &mut flowtorrent::app::AppHandle,
    predicate: impl Fn(&flowtorrent::app::AppSnapshot) -> bool,
) -> anyhow::Result<flowtorrent::app::AppSnapshot> {
    tokio::time::timeout(Duration::from_secs(3), lifecycle::until(handle, predicate)).await?
}

#[test]
fn magnet_input_keeps_discovery_fields_but_ignores_untrusted_selection_ranges() -> anyhow::Result<()>
{
    let normalized = normalize_magnet(
        "  magnet:?xt=urn:btih:0123456789012345678901234567890123456789&dn=Example&tr=udp%3A%2F%2Flocalhost%3A9999&so=0-18446744073709551615&xs=https%3A%2F%2Fexample.com  ",
    )?;
    let parsed = librqbit::Magnet::parse(&normalized)?;
    assert_eq!(
        parsed.as_id20().unwrap().as_string(),
        "0123456789012345678901234567890123456789"
    );
    assert_eq!(parsed.name.as_deref(), Some("Example"));
    assert_eq!(parsed.trackers, vec!["udp://localhost:9999"]);
    assert!(parsed.get_select_only().is_none());
    assert!(!normalized.contains("example.com"));
    Ok(())
}

#[test]
fn invalid_and_oversized_magnet_inputs_are_rejected_before_network_access() {
    for input in [
        "",
        "https://example.com/test.torrent",
        "magnet:?dn=no-hash",
        "magnet:?xt=urn:btih:bad",
        &format!("magnet:?xt={}", "x".repeat(17000)),
    ] {
        assert!(normalize_magnet(input).is_err(), "accepted {input:.80}");
    }
    assert!(matches!(
        normalize_magnet(&format!("magnet:?xt=urn:btmh:1220{}", "1".repeat(64))),
        Err(AppError::UnsupportedFormat)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn magnet_metadata_preview_uses_local_peer_without_payload_or_engine_job()
-> anyhow::Result<()> {
    let source = support::tempdir()?;
    let target = support::tempdir()?;
    let bytes = support::payload();
    let metadata = support::torrent_bytes(&bytes, false);
    let id = librqbit::torrent_from_bytes(&metadata)?.info_hash;
    let (seeder, address) = support::seed(source.path(), &metadata, &bytes).await?;
    let mut engine = LibrqbitEngine::with_options(
        target.path().into(),
        support::local_options(),
        vec![address],
    )
    .await?;
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        engine.inspect_magnet(format!("magnet:?xt=urn:btih:{}", id.as_string())),
    )
    .await?;
    let no_payload = std::fs::read_dir(target.path())?.count() == 0;
    let no_job = engine.snapshot().await?.is_empty();
    engine.shutdown().await?;
    seeder.stop().await;
    let preview = result?;
    assert_eq!(preview.id.0, id.0);
    assert_eq!(preview.name, "fixture.bin");
    assert_eq!(preview.files.len(), 1);
    assert_eq!(preview.files[0].length, 512 * 1024);
    assert!(no_payload, "metadata inspection created payload files");
    assert!(no_job, "metadata inspection registered an engine torrent");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn magnet_preview_waits_for_confirmation_then_downloads_with_existing_flow()
-> anyhow::Result<()> {
    use flowtorrent::{
        app::commands::AppCommand,
        domain::{AddRequest, TransferState},
    };
    let source = support::tempdir()?;
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let bytes = support::payload();
    let metadata = support::torrent_bytes(&bytes, false);
    let id = librqbit::torrent_from_bytes(&metadata)?.info_hash;
    let (seeder, address) = support::seed(source.path(), &metadata, &bytes).await?;
    let mut handle = start(root.path(), vec![address]).await?;
    handle
        .commands
        .send(AppCommand::OpenMagnet(format!(
            "magnet:?xt=urn:btih:{}",
            id.as_string()
        )))
        .await?;
    let preview = within(&mut handle, |s| s.preview.is_some()).await;
    if preview.is_err() {
        lifecycle::stop(handle).await?;
        seeder.stop().await;
        anyhow::bail!("magnet never reached confirmation preview");
    }
    let snapshot = preview?;
    assert!(snapshot.torrents.is_empty());
    assert!(!paths.manifest_file.exists());
    assert_eq!(std::fs::read_dir(&paths.download_dir)?.count(), 0);
    let preview = snapshot.preview.unwrap();
    handle
        .commands
        .send(AppCommand::ConfirmAdd(AddRequest::new(
            preview.id,
            paths.download_dir.clone(),
            preview.default_selection(),
        )))
        .await?;
    lifecycle::until(&mut handle, |s| {
        s.torrents
            .iter()
            .any(|t| matches!(t.state, TransferState::Seeding | TransferState::Completed))
    })
    .await?;
    lifecycle::stop(handle).await?;
    seeder.stop().await;
    assert_eq!(
        std::fs::read(paths.download_dir.join("fixture.bin"))?,
        bytes
    );
    assert!(paths.manifest_file.exists());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_pending_magnet_advances_to_queued_file_without_creating_payload()
-> anyhow::Result<()> {
    use flowtorrent::app::commands::AppCommand;
    let root = support::tempdir()?;
    let paths = lifecycle::paths(root.path());
    let stalled = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let mut handle = start(root.path(), vec![stalled.local_addr()?]).await?;
    handle
        .commands
        .send(AppCommand::OpenMagnet(
            "magnet:?xt=urn:btih:0123456789012345678901234567890123456789".into(),
        ))
        .await?;
    let resolving = within(&mut handle, |s| s.resolving_magnet).await;
    if resolving.is_err() {
        lifecycle::stop(handle).await?;
        anyhow::bail!("magnet lookup did not expose cancellable progress");
    }
    let path = root.path().join("queued.torrent");
    std::fs::write(&path, support::torrent_bytes(&support::payload(), false))?;
    handle.commands.send(AppCommand::OpenTorrent(path)).await?;
    handle.commands.send(AppCommand::CancelPreview).await?;
    let next = within(&mut handle, |s| s.preview.is_some() && !s.resolving_magnet).await?;
    assert_eq!(next.preview.unwrap().name, "fixture.bin");
    handle.commands.send(AppCommand::CancelPreview).await?;
    within(&mut handle, |s| s.preview.is_none() && !s.resolving_magnet).await?;
    lifecycle::stop(handle).await?;
    assert!(!paths.manifest_file.exists());
    assert_eq!(std::fs::read_dir(paths.download_dir)?.count(), 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quit_during_magnet_lookup_does_not_wait_for_metadata_timeout() -> anyhow::Result<()> {
    use flowtorrent::app::commands::AppCommand;
    let root = support::tempdir()?;
    let stalled = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let mut handle = start(root.path(), vec![stalled.local_addr()?]).await?;
    handle
        .commands
        .send(AppCommand::OpenMagnet(
            "magnet:?xt=urn:btih:0123456789012345678901234567890123456789".into(),
        ))
        .await?;
    let resolving = within(&mut handle, |s| s.resolving_magnet).await;
    tokio::time::timeout(Duration::from_secs(3), lifecycle::stop(handle)).await??;
    assert!(resolving.is_ok(), "lookup never entered progress state");
    Ok(())
}

#[test]
fn peer_metadata_nesting_is_rejected_inside_the_network_decoder() -> anyhow::Result<()> {
    // A bounded 140-level fixture and larger disposable stacks make the unfixed
    // decoder return normally, so RED is an assertion failure, never a crash.
    std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| -> anyhow::Result<()> {
        tokio::runtime::Builder::new_multi_thread().worker_threads(2).thread_stack_size(16 * 1024 * 1024).enable_all().build()?.block_on(async {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            use sha1::{Digest, Sha1};
            use librqbit_bencode::{BencodeValue, BencodeValueBorrowed};
            let ordinary = support::torrent_bytes(&[7; 256], false);
            let mut info = b"d9:extension".to_vec();
            info.extend(vec![b'l'; 140]);
            info.extend(b"i0e");
            info.extend(vec![b'e'; 140]);
            info.extend(&ordinary[8..ordinary.len() - 1]);
            let hash = Sha1::digest(&info);
            let hash = hash.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let address = listener.local_addr()?;
            let server = tokio::spawn(async move {
                async fn frame(socket: &mut tokio::net::TcpStream, body: &[u8]) -> anyhow::Result<()> {
                    socket.write_all(&(body.len() as u32).to_be_bytes()).await?;
                    socket.write_all(body).await?;
                    Ok(())
                }
                let (mut socket, _) = listener.accept().await?;
                let mut handshake = [0; 68];
                socket.read_exact(&mut handshake).await?;
                handshake[25] |= 0x10;
                handshake[48..].fill(7);
                socket.write_all(&handshake).await?;
                let hello = format!("d1:md11:ut_metadatai1ee13:metadata_sizei{}ee", info.len());
                let mut body = vec![20, 0];
                body.extend(hello.as_bytes());
                frame(&mut socket, &body).await?;
                let mut response_id = None;
                loop {
                    let length = socket.read_u32().await? as usize;
                    anyhow::ensure!(length <= 16384, "unexpected client message size");
                    let mut message = vec![0; length];
                    socket.read_exact(&mut message).await?;
                    if message.starts_with(&[20, 0]) {
                        let parsed: BencodeValueBorrowed<'_> = librqbit_bencode::from_bytes(&message[2..]).map_err(|error| anyhow::anyhow!("{error}"))?;
                        if let BencodeValue::Dict(root) = parsed {
                            for (key, value) in root {
                                if key.as_ref() == b"m" && let BencodeValue::Dict(methods) = value {
                                    for (name, value) in methods {
                                        if name.as_ref() == b"ut_metadata" && let BencodeValue::Integer(id) = value {
                                            response_id = Some(id as u8);
                                        }
                                    }
                                }
                            }
                        }
                    } else if message.starts_with(&[20, 1]) {
                        let mut response = vec![20, response_id.ok_or_else(|| anyhow::anyhow!("no metadata extension ID"))?];
                        response.extend(format!("d8:msg_typei1e5:piecei0e10:total_sizei{}ee", info.len()).as_bytes());
                        response.extend(&info);
                        frame(&mut socket, &response).await?;
                        let _ = tokio::time::timeout(Duration::from_secs(3), socket.read_u8()).await;
                        return Ok::<_, anyhow::Error>(());
                    }
                }
            });
            let target = support::tempdir()?;
            let mut engine = LibrqbitEngine::with_options(target.path().into(), support::local_options(), vec![address]).await?;
            let result = tokio::time::timeout(Duration::from_secs(5), engine.inspect_magnet(format!("magnet:?xt=urn:btih:{hash}"))).await?;
            tokio::time::timeout(Duration::from_secs(1), server).await???;
            engine.shutdown().await?;
            assert!(matches!(result, Err(AppError::Magnet(_))), "metadata reached the application's late guard instead of failing in the network decoder: {result:?}");
            assert_eq!(std::fs::read_dir(target.path())?.count(), 0);
            Ok(())
        })
    })?.join().map_err(|_| anyhow::anyhow!("metadata test thread panicked"))?
}
