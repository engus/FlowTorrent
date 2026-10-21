#[allow(dead_code)]
mod support;

use anyhow::Result;
use flowtorrent::{
    domain::{AddMode, AddRequest, TorrentId},
    engine::{TorrentEngine, librqbit::LibrqbitEngine},
};
use librqbit::DhtSessionConfig;
use librqbit_bencode::ByteBufOwned;
use serde::Deserialize;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::net::UdpSocket;

#[derive(Deserialize)]
struct Query {
    t: ByteBufOwned,
    #[serde(default)]
    a: Arguments,
}

#[derive(Default, Deserialize)]
struct Arguments {
    info_hash: Option<ByteBufOwned>,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_infohash_is_never_sent_to_dht_with_public_positive_control() -> Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    let addr = socket.local_addr()?;
    let hashes = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
    let observed = hashes.clone();
    let node = tokio::spawn(async move {
        let mut buffer = [0u8; 4096];
        loop {
            let (length, sender) = socket.recv_from(&mut buffer).await?;
            let query: Query = librqbit_bencode::from_bytes(&buffer[..length])
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            if let Some(hash) = query.a.info_hash {
                observed.lock().unwrap().push(hash.as_ref().to_vec());
            }
            let mut reply = b"d1:rd2:id20:".to_vec();
            reply.extend([0x61; 20]);
            reply.extend(b"5:nodes26:");
            reply.extend([0x61; 20]);
            reply.extend([127, 0, 0, 1]);
            reply.extend(addr.port().to_be_bytes());
            reply.extend(b"5:token4:flowe1:t");
            reply.extend(format!("{}:", query.t.as_ref().len()).as_bytes());
            reply.extend(query.t.as_ref());
            reply.extend(b"1:y1:re");
            socket.send_to(&reply, sender).await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    });
    let root = support::tempdir()?;
    for private in [false, true] {
        hashes.lock().unwrap().clear();
        let target = support::tempdir()?;
        let path = root.path().join(if private {
            "private.torrent"
        } else {
            "public.torrent"
        });
        std::fs::write(&path, support::torrent_bytes(&support::payload(), private))?;
        let mut options = support::local_options();
        options.ipv4_only = true;
        options.dht = Some(DhtSessionConfig {
            bootstrap_addrs: Some(vec![addr.to_string()]),
            port: None,
            persistence: None,
        });
        let mut engine =
            LibrqbitEngine::with_options(target.path().into(), options, vec![]).await?;
        let preview = engine.inspect(&path).await?;
        let TorrentId(hash) = preview.id;
        engine
            .add(
                &preview,
                &AddRequest::new(preview.id, target.path().into(), vec![preview.files[0].id]),
                AddMode::New,
            )
            .await?;
        if private {
            tokio::time::sleep(Duration::from_secs(2)).await;
            assert!(
                !hashes.lock().unwrap().iter().any(|sent| sent == &hash),
                "private infohash leaked to DHT"
            );
        } else {
            tokio::time::timeout(Duration::from_secs(15), async {
                while !hashes.lock().unwrap().iter().any(|sent| sent == &hash) {
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
            })
            .await?;
        }
        engine.shutdown().await?;
    }
    node.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_torrent_neither_sends_nor_accepts_pex_with_public_positive_control() -> Result<()>
{
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    for private in [false, true] {
        let peer = TcpListener::bind("127.0.0.1:0").await?;
        let injected = TcpListener::bind("127.0.0.1:0").await?;
        let peer_addr = peer.local_addr()?;
        let injected_addr = injected.local_addr()?;
        let observer = tokio::spawn(async move {
            let (mut connection, _) = peer.accept().await?;
            let mut handshake = [0u8; 68];
            connection.read_exact(&mut handshake).await?;
            assert_eq!(&handshake[1..20], b"BitTorrent protocol");
            handshake[20..28].fill(0);
            handshake[25] = 0x10; // BEP 10 extended messaging.
            handshake[48..68].fill(0x42);
            connection.write_all(&handshake).await?;
            // Advertise all 32 pieces and stay choked so the connection remains live.
            connection
                .write_all(&[0, 0, 0, 5, 5, 255, 255, 255, 255])
                .await?;
            async fn send_frame(stream: &mut tokio::net::TcpStream, payload: &[u8]) -> Result<()> {
                stream
                    .write_all(&(payload.len() as u32).to_be_bytes())
                    .await?;
                stream.write_all(payload).await?;
                Ok(())
            }
            let mut extension = vec![20, 0];
            extension.extend(b"d1:md6:ut_pexi1eee");
            send_frame(&mut connection, &extension).await?;
            let mut pex = vec![20, 1];
            pex.extend(b"d5:added6:");
            pex.extend([127, 0, 0, 1]);
            pex.extend(injected_addr.port().to_be_bytes());
            pex.extend(b"e");
            send_frame(&mut connection, &pex).await?;
            let (mut reader, mut writer) = connection.into_split();
            let keepalive = tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    writer.write_all(&[0, 0, 0, 0]).await?;
                }
                #[allow(unreachable_code)]
                Ok::<(), std::io::Error>(())
            });
            let accepted = tokio::spawn(async move {
                tokio::time::timeout(Duration::from_secs(3), injected.accept())
                    .await
                    .is_ok()
            });
            let sent_pex = tokio::time::timeout(Duration::from_secs(12), async {
                loop {
                    let length = reader.read_u32().await? as usize;
                    anyhow::ensure!(length <= 1024 * 1024, "oversized peer message");
                    let mut frame = vec![0; length];
                    reader.read_exact(&mut frame).await?;
                    if frame.starts_with(&[20, 1]) {
                        return Ok::<_, anyhow::Error>(true);
                    }
                }
            })
            .await;
            let sent_pex = match sent_pex {
                Ok(result) => result?,
                Err(_) => false,
            };
            keepalive.abort();
            Ok::<_, anyhow::Error>((sent_pex, accepted.await?))
        });
        let target = support::tempdir()?;
        let path = target.path().join("pex.torrent");
        std::fs::write(&path, support::torrent_bytes(&support::payload(), private))?;
        let mut engine = LibrqbitEngine::with_options(
            target.path().into(),
            support::local_options(),
            vec![peer_addr],
        )
        .await?;
        let preview = engine.inspect(&path).await?;
        engine
            .add(
                &preview,
                &AddRequest::new(preview.id, target.path().into(), vec![preview.files[0].id]),
                AddMode::New,
            )
            .await?;
        let (sent, accepted) = tokio::time::timeout(Duration::from_secs(20), observer).await???;
        engine.shutdown().await?;
        assert_eq!(sent, !private, "outgoing PEX: private={private}");
        assert_eq!(
            accepted, !private,
            "incoming PEX peer followed: private={private}"
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_infohash_is_not_announced_on_local_multicast() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            "warn,librqbit::session=debug,librqbit_lsd=trace,librqbit_dualstack_sockets=trace",
        )
        .with_test_writer()
        .with_ansi(false)
        .try_init();
    use librqbit_dualstack_sockets::MulticastUdpSocket;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};
    // Observe with the same address families and interface memberships that
    // LSD actually uses. SessionOptions::ipv4_only does not affect its socket.
    let socket = MulticastUdpSocket::new(
        (Ipv6Addr::UNSPECIFIED, 6771).into(),
        SocketAddrV4::new(Ipv4Addr::new(239, 192, 152, 143), 6771),
        SocketAddrV6::new(
            Ipv6Addr::new(0xff15, 0, 0, 0, 0, 0, 0xefc0, 0x988f),
            6771,
            0,
            0,
        ),
        None,
        None,
    )
    .await?;
    eprintln!("LSD observer interfaces: {:?}", socket.nics());
    let root = support::tempdir()?;
    for private in [false, true] {
        let target = support::tempdir()?;
        let path = root.path().join(if private {
            "local-private.torrent"
        } else {
            "local-public.torrent"
        });
        std::fs::write(&path, support::torrent_bytes(&support::payload(), private))?;
        let mut options = support::local_options();
        options.disable_local_service_discovery = false;
        // Match the IPv4 observer explicitly instead of relying on
        // platform-specific dualstack multicast behavior.
        options.ipv4_only = true;
        options.listen.as_mut().unwrap().announce_port = Some(4240);
        let mut engine =
            LibrqbitEngine::with_options(target.path().into(), options, vec![]).await?;
        let preview = engine.inspect(&path).await?;
        let expected = preview
            .id
            .0
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        engine
            .add(
                &preview,
                &AddRequest::new(preview.id, target.path().into(), vec![preview.files[0].id]),
                AddMode::New,
            )
            .await?;
        let found =
            tokio::time::timeout(Duration::from_secs(if private { 2 } else { 15 }), async {
                let mut buffer = [0u8; 4096];
                loop {
                    let (length, _) = socket.recv_from(&mut buffer).await?;
                    if String::from_utf8_lossy(&buffer[..length])
                        .to_ascii_lowercase()
                        .contains(&expected)
                    {
                        return Ok::<_, std::io::Error>(());
                    }
                }
            })
            .await;
        engine.shutdown().await?;
        if private {
            assert!(found.is_err(), "private infohash leaked to local discovery");
        } else {
            found??;
        }
    }
    Ok(())
}
