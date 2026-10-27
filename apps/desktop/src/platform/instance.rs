//! One process owns the engine; other launches deliver bounded structured paths.
use crate::{error::AppResult, paths::AppPaths};
use interprocess::local_socket::{
    ListenerOptions, Name,
    tokio::{Stream, prelude::*},
};
use serde::{Deserialize, Serialize};
use single_instance::SingleInstance;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
    task::JoinHandle,
};

const MAX_BYTES: usize = 256 * 1024;
const MAX_PATHS: usize = 64;
const IO_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenMessage {
    version: u8,
    paths: Vec<PathBuf>,
}
pub enum InstanceRole {
    Primary(InstanceGuard),
    Secondary,
}
pub struct InstanceGuard {
    owner: Arc<SingleInstance>,
    endpoint: String,
    task: Option<JoinHandle<()>>,
}
struct GuardedListener {
    // Rust drops fields in declaration order: release the socket/name before
    // releasing the process guard, even when a listener task is cancelled.
    listener: interprocess::local_socket::tokio::Listener,
    _owner: Arc<SingleInstance>,
}
impl GuardedListener {
    async fn accept(&self) -> std::io::Result<Stream> {
        self.listener.accept().await
    }
}
impl InstanceGuard {
    pub fn acquire(paths: &AppPaths) -> AppResult<InstanceRole> {
        std::fs::create_dir_all(&paths.data_dir)?;
        let endpoint = endpoint(paths)?;
        #[cfg(target_os = "macos")]
        let lock_name = paths
            .data_dir
            .join("instance.lock")
            .to_string_lossy()
            .into_owned();
        #[cfg(not(target_os = "macos"))]
        let lock_name = format!("FlowTorrent-{}", endpoint.replace('/', "-"));
        let owner = SingleInstance::new(&lock_name).map_err(anyhow::Error::from)?;
        if !owner.is_single() {
            return Ok(InstanceRole::Secondary);
        }
        Ok(InstanceRole::Primary(Self {
            owner: Arc::new(owner),
            endpoint,
            task: None,
        }))
    }
    pub fn endpoint_name(&self) -> &str {
        &self.endpoint
    }
    pub fn connection_name(&self) -> std::io::Result<Name<'_>> {
        local_name(&self.endpoint)
    }
    /// Bind before starting the engine. The process guard is retained by the listener
    /// until cancellation has actually dropped its socket, preventing teardown races.
    pub fn listen(&mut self) -> AppResult<mpsc::Receiver<Vec<PathBuf>>> {
        require(self.task.is_none(), "Instance listener is already running")?;
        let name = local_name(&self.endpoint)?;
        let options = ListenerOptions::new().name(name);
        #[cfg(windows)]
        let options = {
            use interprocess::os::windows::{
                local_socket::ListenerOptionsExt, security_descriptor::SecurityDescriptor,
            };
            // Protected DACL: only the owner of this new pipe receives access.
            // No Everyone/Users/anonymous/network access is granted.
            let sddl =
                widestring::U16CString::from_str("D:P(A;;GA;;;OW)").map_err(anyhow::Error::from)?;
            options.security_descriptor(SecurityDescriptor::deserialize(&sddl)?)
        };
        // A stale Unix socket can remain after a crash. Only the guard owner may
        // reclaim it, and its containing directory is verified private to this UID.
        #[cfg(unix)]
        let options = options
            .try_overwrite(true)
            .max_spin_time(Duration::from_millis(100));
        let listener = GuardedListener {
            listener: options.create_tokio()?,
            _owner: self.owner.clone(),
        };
        let (sender, receiver) = mpsc::channel(32);
        self.task = Some(tokio::spawn(async move {
            loop {
                let mut conn = match listener.accept().await {
                    Ok(conn) => conn,
                    Err(error) => {
                        tracing::warn!(%error, "Local open request failed");
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        continue;
                    }
                };
                let result = tokio::time::timeout(IO_TIMEOUT, receive(&mut conn, &sender)).await;
                if let Ok(Err(error)) = result {
                    tracing::debug!(%error, "Rejected local open request");
                }
            }
        }));
        Ok(receiver)
    }
    pub async fn stop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
    }
}
impl Drop for InstanceGuard {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

fn valid_paths(paths: &[PathBuf]) -> bool {
    paths.len() <= MAX_PATHS
        && paths.iter().all(|path| {
            path.is_absolute()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("torrent"))
                && !path.as_os_str().to_string_lossy().contains('\0')
        })
}
async fn receive(conn: &mut Stream, sender: &mpsc::Sender<Vec<PathBuf>>) -> std::io::Result<()> {
    let size = conn.read_u32().await? as usize;
    if size > MAX_BYTES {
        conn.write_u8(0).await?;
        return Ok(());
    }
    let mut bytes = vec![0; size];
    conn.read_exact(&mut bytes).await?;
    let message = serde_json::from_slice::<OpenMessage>(&bytes);
    let status = match message {
        Ok(message) if message.version == 1 && valid_paths(&message.paths) => {
            if sender.try_send(message.paths).is_ok() {
                1
            } else {
                2
            }
        }
        _ => 0,
    };
    conn.write_u8(status).await
}
pub async fn forward_open(paths: &[PathBuf]) -> AppResult<()> {
    forward_open_to(&AppPaths::discover()?, paths).await
}
pub async fn forward_open_to(app_paths: &AppPaths, paths: &[PathBuf]) -> AppResult<()> {
    require(
        valid_paths(paths),
        "Invalid local torrent paths (maximum 64 per launch)",
    )?;
    let bytes = serde_json::to_vec(&OpenMessage {
        version: 1,
        paths: paths.to_vec(),
    })?;
    require(bytes.len() <= MAX_BYTES, "Local open request is too large")?;
    let endpoint = endpoint(app_paths)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut conn = loop {
        let result = tokio::time::timeout(
            Duration::from_millis(250),
            Stream::connect(local_name(&endpoint)?),
        )
        .await;
        match result {
            Ok(Ok(conn)) => break conn,
            result if tokio::time::Instant::now() >= deadline => {
                return Err(anyhow::anyhow!(
                    "The running FlowTorrent instance did not become ready: {result:?}"
                )
                .into());
            }
            _ => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    };
    // Never reconnect/retry after sending: an uncertain acknowledgement must not
    // duplicate a batch the primary may already have accepted.
    tokio::time::timeout(IO_TIMEOUT, async {
        conn.write_all(&(bytes.len() as u32).to_be_bytes()).await?;
        conn.write_all(&bytes).await?;
        anyhow::ensure!(
            conn.read_u8().await? == 1,
            "The running instance rejected the open request or its queue is full"
        );
        Ok::<_, anyhow::Error>(())
    })
    .await
    .map_err(anyhow::Error::from)??;
    Ok(())
}
fn endpoint(paths: &AppPaths) -> AppResult<String> {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt;
        paths.data_dir.as_os_str().as_bytes().to_vec()
    };
    #[cfg(windows)]
    let bytes = {
        use std::os::windows::ffi::OsStrExt;
        paths
            .data_dir
            .as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    let hash = stable_namespace(&bytes);
    #[cfg(windows)]
    {
        Ok(format!("FlowTorrent-{hash:016x}"))
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        // SAFETY: geteuid has no pointer arguments or preconditions.
        let uid = unsafe { libc::geteuid() };
        #[cfg(target_os = "macos")]
        let parent = private_macos_temp(uid)?;
        #[cfg(not(target_os = "macos"))]
        let parent = std::env::temp_dir();
        let directory = parent.join(format!("ft-{uid}-{hash:016x}"));
        match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        let meta = std::fs::symlink_metadata(&directory)?;
        require(
            meta.is_dir()
                && !meta.file_type().is_symlink()
                && meta.uid() == uid
                && meta.mode() & 0o777 == 0o700,
            "Unsafe local IPC directory",
        )?;
        let socket = directory.join("open.sock");
        require(socket.as_os_str().len() < 104, "Local IPC path is too long")?;
        Ok(socket.to_string_lossy().into_owned())
    }
}
fn stable_namespace(bytes: &[u8]) -> u64 {
    // FNV-1a is a fixed namespace protocol, not an authentication mechanism.
    // Privacy comes from the containing directory / Windows owner-only DACL.
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

#[cfg(target_os = "macos")]
fn private_macos_temp(uid: libc::uid_t) -> AppResult<PathBuf> {
    use std::os::unix::{ffi::OsStringExt, fs::MetadataExt};
    // Use the OS's protected per-user root rather than a shared predictable
    // /tmp name or an untrusted TMPDIR override.
    // SAFETY: a null buffer with size0 asks confstr for the required length.
    let length = unsafe { libc::confstr(libc::_CS_DARWIN_USER_TEMP_DIR, std::ptr::null_mut(), 0) };
    require(
        length > 1 && length <= 4096,
        "Cannot discover the private Mac temporary directory",
    )?;
    let mut bytes = vec![0; length];
    // SAFETY: bytes is a writable allocation of exactly the supplied size.
    let written = unsafe {
        libc::confstr(
            libc::_CS_DARWIN_USER_TEMP_DIR,
            bytes.as_mut_ptr().cast(),
            bytes.len(),
        )
    };
    require(
        written == length && bytes.last() == Some(&0),
        "Cannot read the private Mac temporary directory",
    )?;
    bytes.pop();
    let parent: PathBuf = PathBuf::from(std::ffi::OsString::from_vec(bytes))
        .components()
        .collect();
    let metadata = std::fs::symlink_metadata(&parent)?;
    require(
        metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && metadata.uid() == uid
            && metadata.mode() & 0o777 == 0o700,
        "Unsafe Mac temporary directory",
    )?;
    Ok(parent)
}

fn local_name(endpoint: &str) -> std::io::Result<Name<'_>> {
    #[cfg(windows)]
    {
        endpoint.to_ns_name::<interprocess::local_socket::GenericNamespaced>()
    }
    #[cfg(unix)]
    {
        endpoint.to_fs_name::<interprocess::local_socket::GenericFilePath>()
    }
}

fn require(condition: bool, message: &str) -> AppResult<()> {
    if condition {
        Ok(())
    } else {
        Err(anyhow::anyhow!(message.to_owned()).into())
    }
}

#[cfg(test)]
mod namespace_tests {
    #[test]
    fn namespace_bytes_have_a_version_independent_protocol_identifier() {
        assert_eq!(super::stable_namespace(b"hello"), 0xa430d84680aabd0b);
        assert_eq!(super::stable_namespace(b""), 0xcbf29ce484222325);
    }
}
