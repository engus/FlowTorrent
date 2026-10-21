//! Faults wrap the actual capability-backed storage, never fill a host disk.
use super::*;
#[path = "../../tests/support/mod.rs"]
#[allow(dead_code)]
mod support;
use anyhow::Result;
use librqbit::{
    ManagedTorrentShared, TorrentMetadata,
    storage::{BoxStorageFactory, StorageFactory, TorrentStorage},
};
use std::{io::ErrorKind, time::Duration};

#[cfg(unix)]
#[test]
fn multifile_add_survives_a_low_inherited_file_descriptor_limit() -> Result<()> {
    let root = support::tempdir()?;
    std::fs::write(
        root.path().join("nofile.fixture"),
        b"flowtorrent-nofile-fixture",
    )?;
    let child = std::process::Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "engine::librqbit::fault_tests::low_nofile_child_fixture",
            "--nocapture",
        ])
        .env("FLOWTORRENT_NOFILE_ROOT", root.path())
        .output()?;
    anyhow::ensure!(
        child.status.success(),
        "limited-fd child failed: {}\n{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("downloads/bundle"))?.count(),
        256
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn low_nofile_child_fixture() -> Result<()> {
    let Some(root) = std::env::var_os("FLOWTORRENT_NOFILE_ROOT") else {
        return Ok(());
    };
    let root = PathBuf::from(root);
    anyhow::ensure!(
        root.is_absolute()
            && std::fs::read(root.join("nofile.fixture"))? == b"flowtorrent-nofile-fixture",
        "missing fixture ownership marker"
    );
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    // Change only this disposable child, never the parent test process or OS configuration.
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: initialized writable rlimit and the documented resource constant.
    anyhow::ensure!(
        unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } == 0,
        "getrlimit failed"
    );
    anyhow::ensure!(
        limit.rlim_max >= 512,
        "fixture requires an adequate hard limit"
    );
    limit.rlim_cur = 64;
    // SAFETY: retain the inherited hard limit, lowering only the owned child's soft limit.
    anyhow::ensure!(
        unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) } == 0,
        "setrlimit failed"
    );
    runtime.block_on(async {
        use sha1::{Digest, Sha1};
        let mut bytes = b"d4:infod5:filesl".to_vec();
        for index in 0..256 {
            bytes.extend(format!("d6:lengthi1e4:pathl7:{index:03}.binee").as_bytes());
        }
        bytes.extend(b"e4:name6:bundle12:piece lengthi16384e6:pieces20:");
        bytes.extend(Sha1::digest([7; 256]));
        bytes.extend(b"ee");
        let preview = parse_preview(bytes)?;
        let destination = root.join("downloads");
        std::fs::create_dir(&destination)?;
        let mut engine =
            LibrqbitEngine::with_options(destination.clone(), support::local_options(), vec![])
                .await?;
        let result = engine
            .add(
                &preview,
                &AddRequest::new(preview.id, destination, preview.default_selection()),
                AddMode::New,
            )
            .await;
        engine.shutdown().await?;
        result.context("256-file torrent rejected under inherited soft limit64")?;
        Ok(())
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_seeded_jobs_share_one_global_upload_limit() -> Result<()> {
    // Catches omitting the upload limit or applying a full limit per torrent.
    use std::{io::Write, num::NonZeroU32, time::Instant};
    let root = support::tempdir()?;
    let payload = support::payload().repeat(4);
    let mut engine =
        LibrqbitEngine::with_options(root.path().into(), support::local_options(), vec![]).await?;
    let mut metainfo = Vec::new();
    for (index, private) in [false, true].into_iter().enumerate() {
        let preview = parse_preview(support::torrent_bytes(&payload, private))?;
        let folder = root.path().join(format!("seed{index}"));
        std::fs::create_dir(&folder)?;
        let mut request = AddRequest::new(preview.id, folder, preview.default_selection());
        let prepared = Arc::new(prepare_add(&preview, &request)?);
        prepared.claim_files()?;
        let file = prepared
            .file_handles()?
            .remove(0)
            .context("owned seed file")?;
        file.lock().unwrap().writer()?.write_all(&payload)?;
        request.prepared = Some(prepared);
        request.start_paused = true;
        engine.add(&preview, &request, AddMode::New).await?;
        metainfo.push(preview);
    }
    rows_until(&engine, |rows| {
        rows.iter().all(|r| r.state == TransferState::Completed)
    })
    .await?;
    engine
        .set_limits(TransferLimits {
            download_bps: None,
            upload_bps: NonZeroU32::new(65_536),
        })
        .await?;
    for preview in &metainfo {
        engine.resume(preview.id).await?;
    }
    let peer = engine.session.listen_addr().context("seed listener")?;
    let mut sessions = Vec::new();
    let mut downloads = Vec::new();
    for (index, preview) in metainfo.into_iter().enumerate() {
        let folder = root.path().join(format!("download{index}"));
        std::fs::create_dir(&folder)?;
        let session = Session::new_with_opts(folder, support::local_options()).await?;
        let handle = session
            .add_torrent(
                AddTorrent::from_bytes(preview.metadata),
                Some(AddTorrentOptions {
                    initial_peers: Some(vec![peer]),
                    ..Default::default()
                }),
            )
            .await?
            .into_handle()
            .context("downloader")?;
        sessions.push(session);
        downloads.push(handle);
    }
    let progress = || {
        downloads
            .iter()
            .map(|h| h.stats().progress_bytes)
            .sum::<u64>()
    };
    tokio::time::timeout(Duration::from_secs(20), async {
        while progress() < 131_072 {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await?;
    let baseline = progress();
    let start = Instant::now();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let delta = progress() - baseline;
    let elapsed = start.elapsed();
    for session in sessions {
        session.stop().await;
    }
    engine.shutdown().await?;
    assert!(delta >= 16_384, "positive upload control: {delta}");
    assert!(
        delta as f64 <= elapsed.as_secs_f64() * 65_536.0 + 49_152.0,
        "aggregate upload exceeded one shared limit: {delta}"
    );
    Ok(())
}

#[derive(Clone)]
struct FailingFactory {
    inner: SecureStorageFactory,
    failed: Arc<AtomicBool>,
    kind: ErrorKind,
}
impl StorageFactory for FailingFactory {
    type Storage = FailingStorage;
    fn create(
        &self,
        shared: &ManagedTorrentShared,
        metadata: &TorrentMetadata,
    ) -> Result<Self::Storage> {
        Ok(FailingStorage {
            inner: Box::new(self.inner.create(shared, metadata)?),
            failed: self.failed.clone(),
            kind: self.kind,
        })
    }
    fn clone_box(&self) -> BoxStorageFactory {
        self.clone().boxed()
    }
}
struct FailingStorage {
    inner: Box<dyn TorrentStorage>,
    failed: Arc<AtomicBool>,
    kind: ErrorKind,
}
impl TorrentStorage for FailingStorage {
    fn init(&mut self, shared: &ManagedTorrentShared, metadata: &TorrentMetadata) -> Result<()> {
        self.inner.init(shared, metadata)
    }
    fn pread_exact(&self, id: usize, offset: u64, buffer: &mut [u8]) -> Result<()> {
        self.inner.pread_exact(id, offset, buffer)
    }
    fn pwrite_all(&self, id: usize, offset: u64, buffer: &[u8]) -> Result<()> {
        if self.failed.load(Ordering::SeqCst) {
            return Err(std::io::Error::new(self.kind, "controlled payload write failure").into());
        }
        self.inner.pwrite_all(id, offset, buffer)
    }
    fn ensure_file_length(&self, id: usize, length: u64) -> Result<()> {
        self.inner.ensure_file_length(id, length)
    }
    fn remove_file(&self, id: usize, filename: &Path) -> Result<()> {
        self.inner.remove_file(id, filename)
    }
    fn remove_directory_if_empty(&self, path: &Path) -> Result<()> {
        self.inner.remove_directory_if_empty(path)
    }
    fn take(&self) -> Result<Box<dyn TorrentStorage>> {
        Ok(Box::new(Self {
            inner: self.inner.take()?,
            failed: self.failed.clone(),
            kind: self.kind,
        }))
    }
}

async fn rows_until(
    engine: &LibrqbitEngine,
    condition: impl Fn(&[TorrentSnapshot]) -> bool,
) -> Result<Vec<TorrentSnapshot>> {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let rows = engine.snapshot().await?;
            if condition(&rows) {
                return Ok(rows);
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await?
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn payload_write_fault_is_actionable_isolated_and_recoverable() -> Result<()> {
    // Catches losing typed IO errors or treating one storage fault as session-wide.
    for (kind, code) in [
        (ErrorKind::StorageFull, ErrorCode::DiskFull),
        (ErrorKind::PermissionDenied, ErrorCode::PermissionDenied),
    ] {
        let source = support::tempdir()?;
        let target = support::tempdir()?;
        let payload = support::payload();
        let metadata = support::torrent_bytes(&payload, false);
        let private = support::torrent_bytes(&payload, true);
        let (seeder, peer) = support::seed(source.path(), &metadata, &payload).await?;
        let seed = seeder
            .add_torrent(
                AddTorrent::from_bytes(private.clone()),
                Some(AddTorrentOptions {
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await?
            .into_handle()
            .context("private seed")?;
        tokio::time::timeout(Duration::from_secs(15), seed.wait_until_completed()).await??;
        let mut engine = LibrqbitEngine::with_options(
            target.path().into(),
            support::local_options(),
            vec![peer],
        )
        .await?;
        let preview = parse_preview(metadata)?;
        let bad_dir = target.path().join("faulted");
        std::fs::create_dir(&bad_dir)?;
        let request = AddRequest::new(preview.id, bad_dir.clone(), preview.default_selection());
        let prepared = Arc::new(prepare_add(&preview, &request)?);
        prepared.claim_files()?;
        let failed = Arc::new(AtomicBool::new(false));
        // Test-only wrapper at the upstream storage boundary. The inner factory
        // and all real hashing, peer IO, state transitions and snapshots run normally.
        let handle = engine
            .session
            .add_torrent(
                AddTorrent::from_bytes(preview.metadata.clone()),
                Some(AddTorrentOptions {
                    output_folder: Some(prepared.destination.to_str().context("path")?.into()),
                    only_files: Some(vec![0]),
                    paused: true,
                    initial_peers: Some(vec![peer]),
                    storage_factory: Some(
                        FailingFactory {
                            inner: SecureStorageFactory::new(prepared.clone()),
                            failed: failed.clone(),
                            kind,
                        }
                        .boxed(),
                    ),
                    ..Default::default()
                }),
            )
            .await?
            .into_handle()
            .context("faulted torrent")?;
        engine.torrents.insert(
            preview.id,
            EngineTorrent {
                handle,
                prepared,
                readers_only: AtomicBool::new(false),
            },
        );
        rows_until(&engine, |rows| rows[0].state == TransferState::Paused).await?;
        failed.store(true, Ordering::SeqCst);
        engine.resume(preview.id).await?;
        let good = parse_preview(private)?;
        let good_dir = target.path().join("healthy");
        std::fs::create_dir(&good_dir)?;
        engine
            .add(
                &good,
                &AddRequest::new(good.id, good_dir.clone(), good.default_selection()),
                AddMode::New,
            )
            .await?;
        let rows = rows_until(&engine, |rows| {
            rows.iter()
                .any(|r| r.id == preview.id && r.state == TransferState::Error)
                && rows
                    .iter()
                    .any(|r| r.id == good.id && r.state == TransferState::Seeding)
        })
        .await?;
        assert_eq!(
            rows.iter()
                .find(|r| r.id == preview.id)
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .code,
            code
        );
        assert_eq!(std::fs::read(good_dir.join("fixture.bin"))?, payload);
        assert!(
            std::fs::read(bad_dir.join("fixture.bin"))?
                .iter()
                .all(|byte| *byte == 0)
        );
        engine.forget(preview.id).await?;
        engine
            .add(&preview, &request, AddMode::RestorePaused)
            .await?;
        rows_until(&engine, |rows| {
            rows.iter()
                .any(|r| r.id == preview.id && r.state == TransferState::Paused)
        })
        .await?;
        engine.resume(preview.id).await?;
        rows_until(&engine, |rows| {
            rows.iter().all(|r| r.state == TransferState::Seeding)
        })
        .await?;
        assert_eq!(std::fs::read(bad_dir.join("fixture.bin"))?, payload);
        engine.shutdown().await?;
        seeder.stop().await;
    }
    Ok(())
}
