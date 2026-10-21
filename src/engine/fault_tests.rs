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
