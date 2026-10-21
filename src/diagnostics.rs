//! Local, bounded, best-effort diagnostics. Never install a network subscriber.
use crate::error::AppResult;
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tracing_subscriber::fmt::MakeWriter;
const FILE_LIMIT: u64 = 1024 * 1024;
const ENTRY_LIMIT: usize = 8192;
#[derive(Clone)]
pub struct BoundedLog {
    inner: Arc<Mutex<RotatingLog>>,
}
struct RotatingLog {
    path: PathBuf,
    backup: PathBuf,
    file: Option<File>,
    length: u64,
}
impl BoundedLog {
    pub fn open(directory: PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(&directory)?;
        let path = directory.join("flowtorrent.log");
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let mut length = file.metadata()?.len();
        if length > FILE_LIMIT {
            file.set_len(0)?;
            length = 0;
        }
        Ok(Self {
            inner: Arc::new(Mutex::new(RotatingLog {
                path,
                backup: directory.join("flowtorrent.log.1"),
                file: Some(file),
                length,
            })),
        })
    }
}
pub struct LogEntry {
    log: BoundedLog,
    bytes: Vec<u8>,
    truncated: bool,
}
impl<'a> MakeWriter<'a> for BoundedLog {
    type Writer = LogEntry;
    fn make_writer(&'a self) -> Self::Writer {
        LogEntry {
            log: self.clone(),
            bytes: Vec::new(),
            truncated: false,
        }
    }
}
impl Write for LogEntry {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let count = buffer
            .len()
            .min(ENTRY_LIMIT.saturating_sub(self.bytes.len()));
        self.bytes.extend_from_slice(&buffer[..count]);
        self.truncated |= count < buffer.len();
        Ok(buffer.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Drop for LogEntry {
    fn drop(&mut self) {
        if self.bytes.is_empty() {
            return;
        }
        if self.truncated {
            self.bytes.truncate(ENTRY_LIMIT - 14);
            if let Err(error) = std::str::from_utf8(&self.bytes) {
                self.bytes.truncate(error.valid_up_to());
            }
            self.bytes.extend_from_slice(b"\n[truncated]\n\n");
        } else if !self.bytes.ends_with(b"\n") {
            self.bytes.push(b'\n');
        }
        if let Ok(mut log) = self.log.inner.lock() {
            let _ = log.append(&self.bytes);
        }
    }
}
impl RotatingLog {
    fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.length + bytes.len() as u64 > FILE_LIMIT {
            drop(self.file.take());
            match std::fs::remove_file(&self.backup) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            std::fs::rename(&self.path, &self.backup)?;
            self.file = Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)?,
            );
            self.length = 0;
        }
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("Local log is unavailable"))?;
        file.write_all(bytes)?;
        self.length += bytes.len() as u64;
        Ok(())
    }
}
pub fn initialize(directory: PathBuf) -> AppResult<()> {
    let writer = BoundedLog::open(directory)?;
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true)
        .with_env_filter(tracing_subscriber::EnvFilter::new("off,flowtorrent=warn"))
        .try_init()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(())
}
