//! Local-network clipboard sync.
//!
//! Clipmo discovers peers with a short pairing code and exchanges framed
//! messages over TCP. All network work is best-effort and runs away from the
//! clipboard listener so local capture can never be blocked by a slow peer.

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex as StdMutex};
use std::time::Duration;

use parking_lot::{Mutex, RwLock};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::db::Db;
use crate::error::{Error, Result};
use crate::models::{
    now_ms, ClipItem, DeviceIdentity, ImageMeta, ItemKind, NewItem, Settings, StoredFile,
    StoredFileStatus, SyncPeer, SyncState, SyncStatus,
};

const PROTOCOL: &str = "clipmo-lan-v2";
const DISCOVERY_PORT: u16 = 47_633;
const FIRST_SYNC_PORT: u16 = 47_634;
const LAST_SYNC_PORT: u16 = 47_644;
const DISCOVERY_TICK: Duration = Duration::from_secs(3);
const WATCH_TICK: Duration = Duration::from_millis(400);
const CONNECT_TIMEOUT: Duration = Duration::from_millis(900);
const IO_TIMEOUT: Duration = Duration::from_secs(20);
const CHUNK_SIZE: usize = 64 * 1024;
const MAX_HEADER_BYTES: usize = 256 * 1024;
const MAX_IMAGE_BYTES: u64 = 512 * 1024;
const HARD_MAX_MESSAGE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncFileMode {
    Allowlist,
    #[default]
    Blocklist,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SyncPreferences {
    pub sync_text: bool,
    pub sync_images: bool,
    pub sync_files: bool,
    pub sync_file_mode: SyncFileMode,
    pub sync_file_extensions: Vec<String>,
    pub sync_max_file_size_mb: u32,
    pub sync_max_total_size_mb: u32,
}

impl Default for SyncPreferences {
    fn default() -> Self {
        Self {
            sync_text: true,
            sync_images: true,
            sync_files: false,
            sync_file_mode: SyncFileMode::Blocklist,
            sync_file_extensions: default_blocklist(),
            sync_max_file_size_mb: 25,
            sync_max_total_size_mb: 100,
        }
    }
}

impl SyncPreferences {
    fn normalized(mut self) -> Self {
        self.sync_file_extensions = normalize_extensions(&self.sync_file_extensions);
        self.sync_max_file_size_mb = self.sync_max_file_size_mb.clamp(1, 1_024);
        self.sync_max_total_size_mb = self.sync_max_total_size_mb.clamp(1, 4_096);
        if self.sync_max_total_size_mb < self.sync_max_file_size_mb {
            self.sync_max_total_size_mb = self.sync_max_file_size_mb;
        }
        self
    }

    fn max_file_bytes(&self) -> u64 {
        u64::from(self.sync_max_file_size_mb) * 1024 * 1024
    }

    fn max_total_bytes(&self) -> u64 {
        u64::from(self.sync_max_total_size_mb) * 1024 * 1024
    }

    fn allows_file_name(&self, name: &str) -> bool {
        let extension = Path::new(name)
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{}", value.to_lowercase()))
            .unwrap_or_default();
        match self.sync_file_mode {
            SyncFileMode::All => true,
            SyncFileMode::Allowlist => self.sync_file_extensions.contains(&extension),
            SyncFileMode::Blocklist => !self.sync_file_extensions.contains(&extension),
        }
    }
}

fn default_blocklist() -> Vec<String> {
    [
        ".exe", ".bat", ".cmd", ".msi", ".scr", ".com", ".cpl", ".dll", ".sys", ".inf", ".vbs",
        ".js", ".jse", ".wsf", ".ps1", ".reg", ".mp4", ".mov", ".avi", ".mkv", ".webm", ".flv",
        ".wmv", ".iso", ".vhd", ".vhdx", ".img", ".dmg", ".zip", ".rar", ".7z", ".tar", ".gz",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn normalize_extensions(values: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = values
        .iter()
        .flat_map(|value| value.split([',', ';', ' ', '\n', '\r', '\t']))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            let value = value.trim_start_matches("*.").trim_start_matches('.');
            format!(".{}", value.to_lowercase())
        })
        .filter(|value| value.len() > 1 && value.len() <= 32)
        .collect();
    normalized.sort();
    normalized.dedup();
    normalized
}

#[derive(Clone)]
pub struct SyncService {
    peers: Arc<RwLock<HashMap<String, PeerRecord>>>,
    queue: Arc<SyncQueue>,
    db: Option<Arc<Db>>,
    store: Option<Arc<SyncStore>>,
    settings: Option<Arc<RwLock<Settings>>>,
    preferences: Arc<RwLock<SyncPreferences>>,
    storage_root: Arc<RwLock<PathBuf>>,
    lamport: Arc<AtomicU64>,
    suppressions: Arc<Mutex<HashMap<String, Suppression>>>,
    listen_port: u16,
}

impl SyncService {
    pub fn inactive() -> Self {
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            queue: Arc::new(SyncQueue::default()),
            db: None,
            store: None,
            settings: None,
            preferences: Arc::new(RwLock::new(SyncPreferences::default())),
            storage_root: Arc::new(RwLock::new(PathBuf::new())),
            lamport: Arc::new(AtomicU64::new(now_ms().unsigned_abs())),
            suppressions: Arc::new(Mutex::new(HashMap::new())),
            listen_port: 0,
        }
    }

    pub fn start(app: AppHandle, db: Arc<Db>, settings: Arc<RwLock<Settings>>) -> io::Result<Self> {
        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| io::Error::other(error.to_string()))?;
        let storage_root = settings
            .read()
            .storage_path
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| data_dir.clone());
        let store = Arc::new(
            SyncStore::open(&data_dir.join("clipdeck.db"))
                .map_err(|error| io::Error::other(error.to_string()))?,
        );
        let preferences = store.load_preferences().unwrap_or_default().normalized();
        let (listener, listen_port) = bind_listener()?;
        let service = Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            queue: Arc::new(SyncQueue::default()),
            db: Some(db),
            store: Some(store),
            settings: Some(settings),
            preferences: Arc::new(RwLock::new(preferences)),
            storage_root: Arc::new(RwLock::new(storage_root)),
            lamport: Arc::new(AtomicU64::new(now_ms().unsigned_abs())),
            suppressions: Arc::new(Mutex::new(HashMap::new())),
            listen_port,
        };

        spawn_tcp_server(listener, service.clone(), app.clone())?;
        spawn_discovery(service.clone(), app.clone())?;
        spawn_sender(service.clone())?;
        spawn_watcher(service.clone())?;
        Ok(service)
    }

    pub fn state(&self, settings: &Settings) -> SyncState {
        SyncState {
            enabled: settings.sync_enabled,
            device: settings.device_identity(),
            pairing_code: settings.sync_pairing_code.clone(),
            peers: self
                .peers
                .read()
                .values()
                .map(|peer| SyncPeer {
                    device: peer.device.clone(),
                    last_seen_at: peer.last_seen_at,
                    status: if now_ms() - peer.last_seen_at > 30_000 {
                        SyncStatus::Offline
                    } else {
                        SyncStatus::Synced
                    },
                })
                .collect(),
        }
    }

    pub fn preferences(&self) -> SyncPreferences {
        self.preferences.read().clone()
    }

    pub fn save_preferences(&self, preferences: SyncPreferences) -> Result<SyncPreferences> {
        let preferences = preferences.normalized();
        if let Some(store) = &self.store {
            store.save_preferences(&preferences)?;
        }
        *self.preferences.write() = preferences.clone();
        Ok(preferences)
    }

    pub fn enqueue_item(&self, item: &ClipItem) {
        let Some(settings) = &self.settings else {
            return;
        };
        if !settings.read().sync_enabled {
            return;
        }
        let preferences = self.preferences.read().clone();
        match item.kind {
            ItemKind::Text | ItemKind::Link | ItemKind::Email | ItemKind::Color
                if preferences.sync_text =>
            {
                self.enqueue_ready_item(item.clone());
            }
            ItemKind::Image if preferences.sync_images => self.enqueue_ready_item(item.clone()),
            ItemKind::Files if preferences.sync_files => {
                if item
                    .file_assets
                    .iter()
                    .any(|asset| asset.status == StoredFileStatus::Ready)
                {
                    self.enqueue_ready_item(item.clone());
                } else {
                    self.defer_file_item(item.id);
                }
            }
            _ => {}
        }
    }

    fn defer_file_item(&self, id: i64) {
        let service = self.clone();
        let _ = std::thread::Builder::new()
            .name("sync-file-wait".into())
            .spawn(move || {
                for _ in 0..40 {
                    std::thread::sleep(Duration::from_millis(150));
                    let Some(db) = &service.db else {
                        return;
                    };
                    let Ok(Some(item)) = db.get(id) else {
                        return;
                    };
                    if item
                        .file_assets
                        .iter()
                        .any(|asset| asset.status == StoredFileStatus::Ready)
                    {
                        service.enqueue_ready_item(item);
                        return;
                    }
                    if item.file_assets.iter().all(|asset| {
                        matches!(
                            asset.status,
                            StoredFileStatus::Skipped | StoredFileStatus::Failed
                        )
                    }) {
                        return;
                    }
                }
                log::warn!(
                    "file sync waited for a local snapshot but no ready file became available"
                );
            });
    }

    fn enqueue_ready_item(&self, item: ClipItem) {
        let Some(store) = &self.store else {
            return;
        };
        let version = self.next_version();
        let record = match store.stamp_item(item.id, &version) {
            Ok(record) => record,
            Err(error) => {
                log::warn!("could not stamp clipboard item for sync: {error}");
                return;
            }
        };
        let preferences = self.preferences.read().clone();
        let Some(job) = build_upsert_job(&item, &record, &preferences) else {
            return;
        };
        self.push_job(job, &preferences);
    }

    fn enqueue_edit(&self, row: &WatchRow) {
        let Some(store) = &self.store else {
            return;
        };
        let Some(settings) = &self.settings else {
            return;
        };
        let preferences = self.preferences.read().clone();
        if !settings.read().sync_enabled || !preferences.sync_text || !is_text_like(row.kind) {
            return;
        }
        let version = self.next_version();
        if let Err(error) = store.stamp_by_hash(&row.id_hash, &version) {
            log::warn!("could not stamp edited item for sync: {error}");
            return;
        }
        self.push_job(
            SyncJob::action(SyncBody::ClipEdit {
                id_hash: row.id_hash.clone(),
                kind: row.kind,
                content: row.content.clone(),
                content_hash: row.content_hash.clone(),
                version,
            }),
            &preferences,
        );
    }

    fn enqueue_favorite(&self, row: &WatchRow) {
        let Some(store) = &self.store else {
            return;
        };
        let Some(settings) = &self.settings else {
            return;
        };
        if !settings.read().sync_enabled {
            return;
        }
        let preferences = self.preferences.read().clone();
        let version = self.next_version();
        if let Err(error) = store.stamp_by_hash(&row.id_hash, &version) {
            log::warn!("could not stamp favorite change for sync: {error}");
            return;
        }
        self.push_job(
            SyncJob::action(SyncBody::FavoriteToggle {
                id_hash: row.id_hash.clone(),
                favorite: row.favorite,
                version,
            }),
            &preferences,
        );
    }

    fn enqueue_tombstone(&self, id_hash: &str) {
        let Some(store) = &self.store else {
            return;
        };
        let Some(settings) = &self.settings else {
            return;
        };
        if !settings.read().sync_enabled {
            return;
        }
        let preferences = self.preferences.read().clone();
        let version = self.next_version();
        if let Err(error) = store.record_tombstone(id_hash, &version) {
            log::warn!("could not record local sync tombstone: {error}");
            return;
        }
        self.push_job(
            SyncJob::action(SyncBody::Tombstone {
                id_hash: id_hash.to_string(),
                version,
            }),
            &preferences,
        );
    }

    fn push_job(&self, job: SyncJob, preferences: &SyncPreferences) {
        if !self.queue.push(job, preferences.max_total_bytes()) {
            log::warn!("sync payload exceeded the configured queue limit and stayed local");
        }
    }

    fn next_version(&self) -> SyncVersion {
        let device_id = self
            .settings
            .as_ref()
            .map(|settings| settings.read().sync_device_id.clone())
            .unwrap_or_else(|| "local".into());
        SyncVersion {
            device_id,
            lamport: self.lamport.fetch_add(1, Ordering::SeqCst) + 1,
            wall_ms: now_ms(),
        }
    }

    fn note_remote_version(&self, version: &SyncVersion) {
        let _ = self.lamport.fetch_max(version.lamport, Ordering::SeqCst);
    }

    fn current_storage_root(&self) -> PathBuf {
        if let Some(settings) = &self.settings {
            if let Some(path) = settings.read().storage_path.as_ref() {
                return PathBuf::from(path);
            }
        }
        self.storage_root.read().clone()
    }

    fn suppress_edit(&self, id_hash: &str, content_hash: &str) {
        self.suppressions
            .lock()
            .entry(id_hash.to_string())
            .or_default()
            .edit_hash = Some(content_hash.to_string());
    }

    fn suppress_favorite(&self, id_hash: &str, favorite: bool) {
        self.suppressions
            .lock()
            .entry(id_hash.to_string())
            .or_default()
            .favorite = Some(favorite);
    }

    fn suppress_assets(&self, id_hash: &str) {
        self.suppressions
            .lock()
            .entry(id_hash.to_string())
            .or_default()
            .assets = true;
    }

    fn suppress_delete(&self, id_hash: &str) {
        self.suppressions
            .lock()
            .entry(id_hash.to_string())
            .or_default()
            .deleted = true;
    }
}

#[cfg(not(test))]
#[tauri::command]
pub async fn load_sync_preferences(
    state: tauri::State<'_, crate::AppState>,
) -> Result<SyncPreferences> {
    Ok(state.sync.preferences())
}

#[cfg(not(test))]
#[tauri::command]
pub async fn save_sync_preferences(
    app: AppHandle,
    state: tauri::State<'_, crate::AppState>,
    preferences: SyncPreferences,
) -> Result<SyncPreferences> {
    let saved = state.sync.save_preferences(preferences)?;
    let _ = app.emit("sync-preferences-updated", &saved);
    Ok(saved)
}

#[derive(Clone)]
struct PeerRecord {
    device: DeviceIdentity,
    address: SocketAddr,
    last_seen_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoveryMessage {
    protocol: String,
    pairing_code: String,
    device: DeviceIdentity,
    tcp_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncEnvelope {
    protocol: String,
    pairing_code: String,
    device: DeviceIdentity,
    tcp_port: u16,
    body: SyncBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum SyncBody {
    ClipUpsert {
        clip: ClipSnapshot,
    },
    ImageUpsert {
        clip: ClipSnapshot,
        image: ImageSnapshot,
    },
    FilesUpsert {
        clip: ClipSnapshot,
        files: Vec<FileSnapshot>,
    },
    ClipEdit {
        id_hash: String,
        kind: ItemKind,
        content: String,
        content_hash: String,
        version: SyncVersion,
    },
    FavoriteToggle {
        id_hash: String,
        favorite: bool,
        version: SyncVersion,
    },
    Tombstone {
        id_hash: String,
        version: SyncVersion,
    },
}

impl SyncBody {
    fn id_hash(&self) -> &str {
        match self {
            Self::ClipUpsert { clip }
            | Self::ImageUpsert { clip, .. }
            | Self::FilesUpsert { clip, .. } => &clip.id_hash,
            Self::ClipEdit { id_hash, .. }
            | Self::FavoriteToggle { id_hash, .. }
            | Self::Tombstone { id_hash, .. } => id_hash,
        }
    }

    fn version(&self) -> &SyncVersion {
        match self {
            Self::ClipUpsert { clip }
            | Self::ImageUpsert { clip, .. }
            | Self::FilesUpsert { clip, .. } => &clip.version,
            Self::ClipEdit { version, .. }
            | Self::FavoriteToggle { version, .. }
            | Self::Tombstone { version, .. } => version,
        }
    }

    fn expected_blob_sizes(&self) -> Vec<u64> {
        match self {
            Self::ImageUpsert { image, .. } => vec![image.image_size, image.thumb_size],
            Self::FilesUpsert { files, .. } => files.iter().map(|file| file.size).collect(),
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClipSnapshot {
    id_hash: String,
    kind: ItemKind,
    preview: String,
    content: String,
    content_hash: String,
    favorite: bool,
    copied_at: i64,
    version: SyncVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageSnapshot {
    extension: String,
    width: u32,
    height: u32,
    image_size: u64,
    thumb_size: u64,
    chunk_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileSnapshot {
    name: String,
    size: u64,
    mime: String,
    chunk_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncVersion {
    device_id: String,
    lamport: u64,
    wall_ms: i64,
}

impl SyncVersion {
    fn newer_than(&self, other: &Self) -> bool {
        (self.lamport, self.wall_ms, self.device_id.as_str())
            > (other.lamport, other.wall_ms, other.device_id.as_str())
    }
}

#[derive(Debug)]
struct SyncJob {
    body: SyncBody,
    blobs: Vec<BlobSource>,
    estimated_bytes: u64,
}

impl SyncJob {
    fn action(body: SyncBody) -> Self {
        Self {
            body,
            blobs: Vec::new(),
            estimated_bytes: 4 * 1024,
        }
    }
}

#[derive(Debug)]
struct BlobSource {
    path: PathBuf,
    size: u64,
}

#[derive(Default)]
struct SyncQueue {
    state: StdMutex<QueueState>,
    ready: Condvar,
}

#[derive(Default)]
struct QueueState {
    jobs: VecDeque<SyncJob>,
    bytes: u64,
}

impl SyncQueue {
    fn push(&self, job: SyncJob, max_bytes: u64) -> bool {
        if job.estimated_bytes > max_bytes {
            return false;
        }
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        while state.bytes.saturating_add(job.estimated_bytes) > max_bytes {
            let Some(dropped) = state.jobs.pop_front() else {
                break;
            };
            state.bytes = state.bytes.saturating_sub(dropped.estimated_bytes);
        }
        state.bytes = state.bytes.saturating_add(job.estimated_bytes);
        state.jobs.push_back(job);
        self.ready.notify_one();
        true
    }

    fn pop(&self) -> SyncJob {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        loop {
            if let Some(job) = state.jobs.pop_front() {
                state.bytes = state.bytes.saturating_sub(job.estimated_bytes);
                return job;
            }
            state = self
                .ready
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }
}

#[derive(Debug, Default)]
struct Suppression {
    edit_hash: Option<String>,
    favorite: Option<bool>,
    assets: bool,
    deleted: bool,
}

fn bind_listener() -> io::Result<(TcpListener, u16)> {
    if let Ok(value) = std::env::var("CLIPMO_SYNC_PORT") {
        if let Ok(port) = value.parse::<u16>() {
            let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))?;
            listener.set_nonblocking(true)?;
            return Ok((listener, port));
        }
    }
    let mut last_error = None;
    for port in FIRST_SYNC_PORT..=LAST_SYNC_PORT {
        match TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)) {
            Ok(listener) => {
                listener.set_nonblocking(true)?;
                return Ok((listener, port));
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::new(io::ErrorKind::AddrInUse, "no sync port")))
}

fn spawn_discovery(service: SyncService, app: AppHandle) -> io::Result<()> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT))?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(Duration::from_millis(500)))?;
    let receiver = socket.try_clone()?;
    let listener_service = service.clone();
    std::thread::Builder::new()
        .name("sync-discovery-listen".into())
        .spawn(move || listen_for_peers(receiver, listener_service, app))?;
    std::thread::Builder::new()
        .name("sync-discovery-send".into())
        .spawn(move || broadcast_presence(socket, service))?;
    Ok(())
}

fn listen_for_peers(socket: UdpSocket, service: SyncService, app: AppHandle) {
    let mut buffer = [0u8; 4096];
    loop {
        let Ok((length, source)) = socket.recv_from(&mut buffer) else {
            continue;
        };
        let Ok(message) = serde_json::from_slice::<DiscoveryMessage>(&buffer[..length]) else {
            continue;
        };
        let Some(settings) = &service.settings else {
            continue;
        };
        let current = settings.read().clone();
        if !current.sync_enabled
            || message.protocol != PROTOCOL
            || message.pairing_code != current.sync_pairing_code
            || message.device.id == current.sync_device_id
            || message.tcp_port == 0
        {
            continue;
        }
        service.peers.write().insert(
            message.device.id.clone(),
            PeerRecord {
                device: message.device,
                address: SocketAddr::new(source.ip(), message.tcp_port),
                last_seen_at: now_ms(),
            },
        );
        let _ = app.emit("sync-peers-updated", ());
    }
}

fn broadcast_presence(socket: UdpSocket, service: SyncService) {
    loop {
        let Some(settings) = &service.settings else {
            return;
        };
        let current = settings.read().clone();
        if current.sync_enabled {
            let message = DiscoveryMessage {
                protocol: PROTOCOL.into(),
                pairing_code: current.sync_pairing_code.clone(),
                device: current.device_identity(),
                tcp_port: service.listen_port,
            };
            if let Ok(bytes) = serde_json::to_vec(&message) {
                let target = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), DISCOVERY_PORT);
                if let Err(error) = socket.send_to(&bytes, target) {
                    log::debug!("sync discovery broadcast failed: {error}");
                }
            }
        }
        std::thread::sleep(DISCOVERY_TICK);
    }
}

fn spawn_tcp_server(listener: TcpListener, service: SyncService, app: AppHandle) -> io::Result<()> {
    std::thread::Builder::new()
        .name("sync-tcp-server".into())
        .spawn(move || loop {
            match listener.accept() {
                Ok((stream, source)) => {
                    let service = service.clone();
                    let app = app.clone();
                    let _ = std::thread::Builder::new()
                        .name("sync-tcp-client".into())
                        .spawn(move || handle_incoming(stream, source, service, app));
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    log::warn!("sync TCP accept failed: {error}");
                    std::thread::sleep(Duration::from_millis(250));
                }
            }
        })?;
    Ok(())
}

fn handle_incoming(
    mut stream: TcpStream,
    source: SocketAddr,
    service: SyncService,
    app: AppHandle,
) {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    let envelope = match read_envelope(&mut stream) {
        Ok(envelope) => envelope,
        Err(error) => {
            log::debug!("invalid sync frame from {source}: {error}");
            return;
        }
    };
    let Some(settings) = &service.settings else {
        return;
    };
    let current = settings.read().clone();
    if !current.sync_enabled
        || envelope.protocol != PROTOCOL
        || envelope.pairing_code != current.sync_pairing_code
        || envelope.device.id == current.sync_device_id
        || envelope.tcp_port == 0
        || envelope.body.version().device_id != envelope.device.id
        || !valid_id_hash(envelope.body.id_hash())
    {
        return;
    }

    service.note_remote_version(envelope.body.version());
    service.peers.write().insert(
        envelope.device.id.clone(),
        PeerRecord {
            device: envelope.device.clone(),
            address: SocketAddr::new(source.ip(), envelope.tcp_port),
            last_seen_at: now_ms(),
        },
    );

    if let Err(error) = apply_incoming(&mut stream, &service, &app, envelope) {
        log::warn!("synced clipboard change could not be applied: {error}");
    }
}

fn read_envelope(stream: &mut TcpStream) -> io::Result<SyncEnvelope> {
    let mut length = [0u8; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > MAX_HEADER_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "sync header size is invalid",
        ));
    }
    let mut header = vec![0u8; length];
    stream.read_exact(&mut header)?;
    serde_json::from_slice(&header)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
}

fn apply_incoming(
    stream: &mut TcpStream,
    service: &SyncService,
    app: &AppHandle,
    envelope: SyncEnvelope,
) -> Result<()> {
    let Some(db) = &service.db else {
        return Ok(());
    };
    let Some(store) = &service.store else {
        return Ok(());
    };
    let preferences = service.preferences.read().clone();
    let id_hash = envelope.body.id_hash().to_string();
    let version = envelope.body.version().clone();

    match envelope.body {
        SyncBody::ClipUpsert { clip } => {
            if !preferences.sync_text || !is_text_like(clip.kind) {
                return Ok(());
            }
            import_text(db, store, service, app, &envelope.device, clip)?;
        }
        SyncBody::ImageUpsert { clip, image } => {
            if !preferences.sync_images || clip.kind != ItemKind::Image {
                return Ok(());
            }
            if !store.should_accept(&id_hash, &version)? {
                return Ok(());
            }
            let total = image.image_size.saturating_add(image.thumb_size);
            if total == 0 || total > MAX_IMAGE_BYTES {
                return Err(Error::Other(
                    "synced image exceeded the 512 KiB limit".into(),
                ));
            }
            let image_bytes = read_blob(stream, image.image_size, MAX_IMAGE_BYTES)?;
            let thumb_bytes = read_blob(stream, image.thumb_size, MAX_IMAGE_BYTES)?;
            import_image(
                db,
                store,
                service,
                app,
                &envelope.device,
                clip,
                image,
                &image_bytes,
                &thumb_bytes,
            )?;
        }
        SyncBody::FilesUpsert { clip, files } => {
            if !preferences.sync_files || clip.kind != ItemKind::Files {
                return Ok(());
            }
            if !store.should_accept(&id_hash, &version)? {
                return Ok(());
            }
            let declared_total = files
                .iter()
                .fold(0u64, |total, file| total.saturating_add(file.size));
            if declared_total > preferences.max_total_bytes()
                || declared_total > HARD_MAX_MESSAGE_BYTES
            {
                return Err(Error::Other(
                    "synced file batch exceeded the receiver limit".into(),
                ));
            }
            let mut accepted = Vec::new();
            for file in files {
                let bytes = read_blob(stream, file.size, HARD_MAX_MESSAGE_BYTES)?;
                if file.size <= preferences.max_file_bytes()
                    && preferences.allows_file_name(&file.name)
                    && !file.name.trim().is_empty()
                {
                    accepted.push((file, bytes));
                } else {
                    log::info!("receiver skipped a synced file blocked by its local policy");
                }
            }
            if !accepted.is_empty() {
                import_files(db, store, service, app, &envelope.device, clip, accepted)?;
            }
        }
        SyncBody::ClipEdit {
            id_hash,
            kind,
            content,
            content_hash,
            version,
        } => {
            if !preferences.sync_text || !is_text_like(kind) {
                return Ok(());
            }
            if !store.should_accept(&id_hash, &version)? {
                return Ok(());
            }
            let Some(record) = store.find_by_hash(&id_hash)? else {
                return Ok(());
            };
            let item = db.update_text_content(record.item_id, &content, kind, &content_hash)?;
            store.stamp_by_hash(&id_hash, &version)?;
            service.suppress_edit(&id_hash, &content_hash);
            let _ = app.emit("clip-updated", &item);
        }
        SyncBody::FavoriteToggle {
            id_hash,
            favorite,
            version,
        } => {
            if !store.should_accept(&id_hash, &version)? {
                return Ok(());
            }
            let Some(record) = store.find_by_hash(&id_hash)? else {
                return Ok(());
            };
            db.set_favorite(record.item_id, favorite)?;
            store.stamp_by_hash(&id_hash, &version)?;
            service.suppress_favorite(&id_hash, favorite);
            if let Some(item) = db.get(record.item_id)? {
                let _ = app.emit("clip-updated", &item);
            }
        }
        SyncBody::Tombstone { id_hash, version } => {
            if !store.should_accept(&id_hash, &version)? {
                return Ok(());
            }
            service.suppress_delete(&id_hash);
            if let Some(record) = store.find_by_hash(&id_hash)? {
                let orphans = db.delete(record.item_id)?;
                cleanup_asset_paths(&service.current_storage_root(), orphans);
            }
            store.record_tombstone(&id_hash, &version)?;
            let _ = app.emit("clip-updated", ());
        }
    }
    let _ = app.emit("sync-peers-updated", ());
    Ok(())
}

fn import_text(
    db: &Db,
    store: &SyncStore,
    service: &SyncService,
    app: &AppHandle,
    device: &DeviceIdentity,
    clip: ClipSnapshot,
) -> Result<()> {
    if !store.should_accept(&clip.id_hash, &clip.version)? {
        return Ok(());
    }
    let item_id = if let Some(record) = store.find_by_hash(&clip.id_hash)? {
        let current = db.get_required(record.item_id)?;
        if current.content != clip.content || current.kind != clip.kind {
            db.update_text_content(record.item_id, &clip.content, clip.kind, &clip.content_hash)?;
        }
        record.item_id
    } else {
        db.upsert(&NewItem {
            kind: clip.kind,
            preview: clip.preview.clone(),
            content: clip.content.clone(),
            size_bytes: clip.content.len().min(i64::MAX as usize) as i64,
            content_hash: clip.content_hash.clone(),
            device: Some(device.clone()),
            sync_status: SyncStatus::Synced,
            ..Default::default()
        })?
        .id()
    };
    db.set_favorite(item_id, clip.favorite)?;
    store.attach(item_id, &clip.id_hash, &clip.version)?;
    service.suppress_edit(&clip.id_hash, &clip.content_hash);
    service.suppress_favorite(&clip.id_hash, clip.favorite);
    if let Some(item) = db.get(item_id)? {
        let _ = app.emit("clip-updated", &item);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn import_image(
    db: &Db,
    store: &SyncStore,
    service: &SyncService,
    app: &AppHandle,
    device: &DeviceIdentity,
    clip: ClipSnapshot,
    image: ImageSnapshot,
    image_bytes: &[u8],
    thumb_bytes: &[u8],
) -> Result<()> {
    let extension = match image.extension.to_lowercase().as_str() {
        "png" => "png",
        "jpg" | "jpeg" => "jpg",
        "webp" => "webp",
        _ => return Err(Error::Other("synced image format is unsupported".into())),
    };
    let root = service.current_storage_root();
    crate::storage::prepare_root(&root)?;
    replace_existing_if_needed(db, store, service, &clip.id_hash)?;

    let device_dir = safe_component(&device.id);
    let hash = safe_component(&clip.id_hash);
    let image_path = crate::storage::image_root(&root)
        .join(&device_dir)
        .join(format!("{hash}.{extension}"));
    let thumb_path = crate::storage::thumb_root(&root)
        .join(&device_dir)
        .join(format!("{hash}.png"));
    write_atomic(&image_path, image_bytes)?;
    write_atomic(&thumb_path, thumb_bytes)?;
    let item_id = db
        .upsert(&NewItem {
            kind: ItemKind::Image,
            preview: clip.preview.clone(),
            image: Some(ImageMeta {
                path: image_path.to_string_lossy().into_owned(),
                thumb_path: thumb_path.to_string_lossy().into_owned(),
                width: image.width,
                height: image.height,
            }),
            size_bytes: image_bytes.len().min(i64::MAX as usize) as i64,
            content_hash: clip.content_hash.clone(),
            device: Some(device.clone()),
            sync_status: SyncStatus::Synced,
            ..Default::default()
        })?
        .id();
    db.set_favorite(item_id, clip.favorite)?;
    store.attach(item_id, &clip.id_hash, &clip.version)?;
    service.suppress_assets(&clip.id_hash);
    if let Some(item) = db.get(item_id)? {
        let _ = app.emit("clip-updated", &item);
    }
    Ok(())
}

fn import_files(
    db: &Db,
    store: &SyncStore,
    service: &SyncService,
    app: &AppHandle,
    device: &DeviceIdentity,
    clip: ClipSnapshot,
    files: Vec<(FileSnapshot, Vec<u8>)>,
) -> Result<()> {
    let root = service.current_storage_root();
    crate::storage::prepare_root(&root)?;
    replace_existing_if_needed(db, store, service, &clip.id_hash)?;

    let group = crate::storage::file_root(&root)
        .join(safe_component(&device.id))
        .join(safe_component(&clip.id_hash));
    if group.exists() {
        std::fs::remove_dir_all(&group)?;
    }
    std::fs::create_dir_all(&group)?;
    let mut assets = Vec::with_capacity(files.len());
    let mut display_names = Vec::with_capacity(files.len());
    let mut total = 0u64;
    for (index, (file, bytes)) in files.into_iter().enumerate() {
        let name = safe_file_name(&file.name);
        let destination = group.join(format!("{index:03}-{name}"));
        write_atomic(&destination, &bytes)?;
        total = total.saturating_add(file.size);
        display_names.push(file.name.clone());
        assets.push(StoredFile {
            original_path: file.name,
            stored_path: Some(destination.to_string_lossy().into_owned()),
            size_bytes: file.size,
            is_directory: false,
            status: StoredFileStatus::Ready,
            message: None,
            thumb_path: None,
        });
    }

    let item_id = db
        .upsert(&NewItem {
            kind: ItemKind::Files,
            preview: clip.preview.clone(),
            content: display_names.join("\n"),
            files: display_names,
            file_assets: assets,
            size_bytes: total.min(i64::MAX as u64) as i64,
            content_hash: clip.content_hash.clone(),
            device: Some(device.clone()),
            sync_status: SyncStatus::Synced,
            ..Default::default()
        })?
        .id();
    db.set_favorite(item_id, clip.favorite)?;
    store.attach(item_id, &clip.id_hash, &clip.version)?;
    service.suppress_assets(&clip.id_hash);
    if let Some(item) = db.get(item_id)? {
        let _ = app.emit("clip-updated", &item);
    }
    Ok(())
}

fn replace_existing_if_needed(
    db: &Db,
    store: &SyncStore,
    service: &SyncService,
    id_hash: &str,
) -> Result<()> {
    if let Some(record) = store.find_by_hash(id_hash)? {
        let orphans = db.delete(record.item_id)?;
        cleanup_asset_paths(&service.current_storage_root(), orphans);
    }
    Ok(())
}

fn cleanup_asset_paths(storage_root: &Path, paths: Vec<String>) {
    for path in paths {
        if let Err(error) = crate::storage::remove_managed_asset(storage_root, Path::new(&path)) {
            log::warn!("could not remove replaced synced asset {path}: {error}");
        }
    }
}

fn read_blob(stream: &mut TcpStream, size: u64, limit: u64) -> Result<Vec<u8>> {
    if size > limit || size > usize::MAX as u64 {
        return Err(Error::Other(
            "synced binary payload exceeded its limit".into(),
        ));
    }
    let mut bytes = vec![0u8; size as usize];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn spawn_sender(service: SyncService) -> io::Result<()> {
    std::thread::Builder::new()
        .name("sync-send".into())
        .spawn(move || loop {
            let job = service.queue.pop();
            let Some(settings) = &service.settings else {
                continue;
            };
            let current = settings.read().clone();
            if !current.sync_enabled {
                continue;
            }
            let envelope = SyncEnvelope {
                protocol: PROTOCOL.into(),
                pairing_code: current.sync_pairing_code.clone(),
                device: current.device_identity(),
                tcp_port: service.listen_port,
                body: job.body.clone(),
            };
            for peer in service.peers.read().values().cloned() {
                if let Err(error) = send_frame(peer.address, &envelope, &job.blobs) {
                    log::debug!("sync send to {} failed: {error}", peer.device.name);
                }
            }
        })?;
    Ok(())
}

fn send_frame(
    address: SocketAddr,
    envelope: &SyncEnvelope,
    blobs: &[BlobSource],
) -> io::Result<()> {
    let expected = envelope.body.expected_blob_sizes();
    if expected.len() != blobs.len()
        || expected
            .iter()
            .zip(blobs)
            .any(|(expected, blob)| *expected != blob.size)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "sync blob manifest does not match its payload",
        ));
    }
    let header = serde_json::to_vec(envelope)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    if header.len() > MAX_HEADER_BYTES || header.len() > u32::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "sync header is too large",
        ));
    }
    let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    stream.write_all(&(header.len() as u32).to_be_bytes())?;
    stream.write_all(&header)?;
    let mut buffer = vec![0u8; CHUNK_SIZE];
    for blob in blobs {
        let mut file = File::open(&blob.path)?;
        if file.metadata()?.len() != blob.size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "sync source file changed before it could be sent",
            ));
        }
        let mut remaining = blob.size;
        while remaining > 0 {
            let read = file.read(&mut buffer[..remaining.min(CHUNK_SIZE as u64) as usize])?;
            if read == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "sync source file ended early",
                ));
            }
            stream.write_all(&buffer[..read])?;
            remaining -= read as u64;
        }
    }
    stream.flush()
}

fn spawn_watcher(service: SyncService) -> io::Result<()> {
    std::thread::Builder::new()
        .name("sync-change-watch".into())
        .spawn(move || {
            let Some(store) = &service.store else {
                return;
            };
            let mut previous = store.watch_rows().unwrap_or_default();
            loop {
                std::thread::sleep(WATCH_TICK);
                let current = match store.watch_rows() {
                    Ok(rows) => rows,
                    Err(error) => {
                        log::debug!("sync watcher could not read clipboard rows: {error}");
                        continue;
                    }
                };

                for (id_hash, row) in &current {
                    let Some(before) = previous.get(id_hash) else {
                        continue;
                    };
                    if (before.content_hash != row.content_hash || before.content != row.content)
                        && !consume_edit_suppression(&service, id_hash, &row.content_hash)
                    {
                        service.enqueue_edit(row);
                    }
                    if before.favorite != row.favorite
                        && !consume_favorite_suppression(&service, id_hash, row.favorite)
                    {
                        service.enqueue_favorite(row);
                    }
                    if before.assets_fingerprint != row.assets_fingerprint
                        && row.kind == ItemKind::Files
                        && !consume_asset_suppression(&service, id_hash)
                    {
                        if let Some(db) = &service.db {
                            if let Ok(Some(item)) = db.get(row.item_id) {
                                service.enqueue_item(&item);
                            }
                        }
                    }
                }

                for id_hash in previous.keys().filter(|key| !current.contains_key(*key)) {
                    if !consume_delete_suppression(&service, id_hash) {
                        service.enqueue_tombstone(id_hash);
                    }
                }
                previous = current;
            }
        })?;
    Ok(())
}

fn consume_edit_suppression(service: &SyncService, id_hash: &str, content_hash: &str) -> bool {
    let mut suppressions = service.suppressions.lock();
    let Some(suppression) = suppressions.get_mut(id_hash) else {
        return false;
    };
    let matched = suppression.edit_hash.as_deref() == Some(content_hash);
    if matched {
        suppression.edit_hash = None;
    }
    cleanup_suppression(&mut suppressions, id_hash);
    matched
}

fn consume_favorite_suppression(service: &SyncService, id_hash: &str, favorite: bool) -> bool {
    let mut suppressions = service.suppressions.lock();
    let Some(suppression) = suppressions.get_mut(id_hash) else {
        return false;
    };
    let matched = suppression.favorite == Some(favorite);
    if matched {
        suppression.favorite = None;
    }
    cleanup_suppression(&mut suppressions, id_hash);
    matched
}

fn consume_asset_suppression(service: &SyncService, id_hash: &str) -> bool {
    let mut suppressions = service.suppressions.lock();
    let Some(suppression) = suppressions.get_mut(id_hash) else {
        return false;
    };
    let matched = suppression.assets;
    suppression.assets = false;
    cleanup_suppression(&mut suppressions, id_hash);
    matched
}

fn consume_delete_suppression(service: &SyncService, id_hash: &str) -> bool {
    let mut suppressions = service.suppressions.lock();
    let Some(suppression) = suppressions.get_mut(id_hash) else {
        return false;
    };
    let matched = suppression.deleted;
    suppression.deleted = false;
    cleanup_suppression(&mut suppressions, id_hash);
    matched
}

fn cleanup_suppression(suppressions: &mut HashMap<String, Suppression>, id_hash: &str) {
    let remove = suppressions.get(id_hash).is_some_and(|value| {
        value.edit_hash.is_none() && value.favorite.is_none() && !value.assets && !value.deleted
    });
    if remove {
        suppressions.remove(id_hash);
    }
}

fn build_upsert_job(
    item: &ClipItem,
    record: &SyncRecord,
    preferences: &SyncPreferences,
) -> Option<SyncJob> {
    let clip = ClipSnapshot {
        id_hash: record.id_hash.clone(),
        kind: item.kind,
        preview: item.preview.clone(),
        content: item.content.clone(),
        content_hash: record.content_hash.clone(),
        favorite: item.favorite,
        copied_at: item.last_copied_at,
        version: record.version.clone(),
    };
    match item.kind {
        ItemKind::Text | ItemKind::Link | ItemKind::Email | ItemKind::Color => {
            Some(SyncJob::action(SyncBody::ClipUpsert { clip }))
        }
        ItemKind::Image => {
            let image = item.image.as_ref()?;
            let image_path = PathBuf::from(&image.path);
            let thumb_path = PathBuf::from(&image.thumb_path);
            let image_size = std::fs::metadata(&image_path).ok()?.len();
            let thumb_size = std::fs::metadata(&thumb_path).ok()?.len();
            let total = image_size.saturating_add(thumb_size);
            if total == 0 || total > MAX_IMAGE_BYTES {
                log::info!("image stayed local because it exceeded the 512 KiB sync limit");
                return None;
            }
            let extension = image_path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("png")
                .to_lowercase();
            Some(SyncJob {
                body: SyncBody::ImageUpsert {
                    clip,
                    image: ImageSnapshot {
                        extension,
                        width: image.width,
                        height: image.height,
                        image_size,
                        thumb_size,
                        chunk_count: chunk_count(total),
                    },
                },
                blobs: vec![
                    BlobSource {
                        path: image_path,
                        size: image_size,
                    },
                    BlobSource {
                        path: thumb_path,
                        size: thumb_size,
                    },
                ],
                estimated_bytes: total.saturating_add(8 * 1024),
            })
        }
        ItemKind::Files => {
            let mut manifests = Vec::new();
            let mut blobs = Vec::new();
            let mut total = 0u64;
            for asset in &item.file_assets {
                if asset.status != StoredFileStatus::Ready || asset.is_directory {
                    continue;
                }
                let path = PathBuf::from(asset.stored_path.as_ref()?);
                let name = Path::new(&asset.original_path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("clipboard-file")
                    .to_string();
                let size = std::fs::metadata(&path).ok()?.len();
                if size == 0
                    || size > preferences.max_file_bytes()
                    || !preferences.allows_file_name(&name)
                {
                    log::info!("file stayed local because it was blocked by sync policy: {name}");
                    continue;
                }
                if total.saturating_add(size) > preferences.max_total_bytes() {
                    log::info!(
                        "remaining files stayed local because the batch sync cap was reached"
                    );
                    break;
                }
                total = total.saturating_add(size);
                manifests.push(FileSnapshot {
                    name: name.clone(),
                    size,
                    mime: mime_for_name(&name).into(),
                    chunk_count: chunk_count(size),
                });
                blobs.push(BlobSource { path, size });
            }
            if manifests.is_empty() {
                return None;
            }
            Some(SyncJob {
                body: SyncBody::FilesUpsert {
                    clip,
                    files: manifests,
                },
                blobs,
                estimated_bytes: total.saturating_add(16 * 1024),
            })
        }
    }
}

fn chunk_count(size: u64) -> u32 {
    size.div_ceil(CHUNK_SIZE as u64).min(u64::from(u32::MAX)) as u32
}

fn is_text_like(kind: ItemKind) -> bool {
    matches!(
        kind,
        ItemKind::Text | ItemKind::Link | ItemKind::Email | ItemKind::Color
    )
}

fn mime_for_name(name: &str) -> &'static str {
    match Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("txt" | "md" | "csv" | "log") => "text/plain",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

fn valid_id_hash(value: &str) -> bool {
    (16..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn safe_component(value: &str) -> String {
    let mut output: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .take(96)
        .collect();
    if output.is_empty() {
        output.push_str("unknown");
    }
    output
}

fn safe_file_name(value: &str) -> String {
    let name = value
        .rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or("clipboard-file");
    let mut output: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | ' ') {
                character
            } else {
                '_'
            }
        })
        .take(180)
        .collect();
    if output.trim_matches(['.', ' ']).is_empty() {
        output = "clipboard-file".into();
    }
    output
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!(
        "{}.part",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("bin")
    ));
    std::fs::write(&temporary, bytes)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    match std::fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = std::fs::remove_file(&temporary);
            Err(error.into())
        }
    }
}

struct SyncStore {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone)]
struct SyncRecord {
    item_id: i64,
    id_hash: String,
    content_hash: String,
    version: SyncVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WatchRow {
    item_id: i64,
    id_hash: String,
    kind: ItemKind,
    content: String,
    content_hash: String,
    favorite: bool,
    assets_fingerprint: String,
}

impl SyncStore {
    fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;
             CREATE TABLE IF NOT EXISTS item_sync (
               item_id INTEGER PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE,
               id_hash TEXT NOT NULL UNIQUE,
               origin_device_id TEXT NOT NULL,
               origin_lamport INTEGER NOT NULL,
               origin_wall_ms INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS sync_tombstones (
               id_hash TEXT PRIMARY KEY,
               origin_device_id TEXT NOT NULL,
               origin_lamport INTEGER NOT NULL,
               origin_wall_ms INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS sync_preferences (
               key TEXT PRIMARY KEY,
               value TEXT NOT NULL
             );
             INSERT OR IGNORE INTO item_sync (
               item_id, id_hash, origin_device_id, origin_lamport, origin_wall_ms
             )
             SELECT id, hash, device_id, 0, last_copied_at FROM items;",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn load_preferences(&self) -> Result<SyncPreferences> {
        let raw: Option<String> = self
            .conn
            .lock()
            .query_row(
                "SELECT value FROM sync_preferences WHERE key = 'preferences'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let preferences = raw
            .as_deref()
            .and_then(|json| serde_json::from_str::<SyncPreferences>(json).ok())
            .unwrap_or_default()
            .normalized();
        Ok(preferences)
    }

    fn save_preferences(&self, preferences: &SyncPreferences) -> Result<()> {
        let value =
            serde_json::to_string(preferences).map_err(|error| Error::Other(error.to_string()))?;
        self.conn.lock().execute(
            "INSERT INTO sync_preferences (key, value) VALUES ('preferences', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![value],
        )?;
        Ok(())
    }

    fn ensure_all_records(conn: &Connection) -> Result<()> {
        conn.execute(
            "INSERT OR IGNORE INTO item_sync (
               item_id, id_hash, origin_device_id, origin_lamport, origin_wall_ms
             )
             SELECT id, hash, device_id, 0, last_copied_at FROM items",
            [],
        )?;
        Ok(())
    }

    fn stamp_item(&self, item_id: i64, version: &SyncVersion) -> Result<SyncRecord> {
        let conn = self.conn.lock();
        Self::ensure_all_records(&conn)?;
        conn.execute(
            "UPDATE item_sync
                SET origin_device_id = ?2, origin_lamport = ?3, origin_wall_ms = ?4
              WHERE item_id = ?1",
            params![
                item_id,
                version.device_id,
                to_sql_i64(version.lamport),
                version.wall_ms
            ],
        )?;
        query_record_by_id(&conn, item_id)?.ok_or(Error::NotFound("sync item"))
    }

    fn stamp_by_hash(&self, id_hash: &str, version: &SyncVersion) -> Result<()> {
        let changed = self.conn.lock().execute(
            "UPDATE item_sync
                SET origin_device_id = ?2, origin_lamport = ?3, origin_wall_ms = ?4
              WHERE id_hash = ?1",
            params![
                id_hash,
                version.device_id,
                to_sql_i64(version.lamport),
                version.wall_ms
            ],
        )?;
        if changed == 0 {
            Err(Error::NotFound("sync item"))
        } else {
            Ok(())
        }
    }

    fn attach(&self, item_id: i64, id_hash: &str, version: &SyncVersion) -> Result<()> {
        let mut conn = self.conn.lock();
        let transaction = conn.transaction()?;
        transaction.execute(
            "DELETE FROM item_sync WHERE id_hash = ?1 AND item_id != ?2",
            params![id_hash, item_id],
        )?;
        transaction.execute(
            "INSERT INTO item_sync (
               item_id, id_hash, origin_device_id, origin_lamport, origin_wall_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(item_id) DO UPDATE SET
               id_hash = excluded.id_hash,
               origin_device_id = excluded.origin_device_id,
               origin_lamport = excluded.origin_lamport,
               origin_wall_ms = excluded.origin_wall_ms",
            params![
                item_id,
                id_hash,
                version.device_id,
                to_sql_i64(version.lamport),
                version.wall_ms
            ],
        )?;
        transaction.execute(
            "DELETE FROM sync_tombstones WHERE id_hash = ?1",
            params![id_hash],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn find_by_hash(&self, id_hash: &str) -> Result<Option<SyncRecord>> {
        query_record_by_hash(&self.conn.lock(), id_hash)
    }

    fn should_accept(&self, id_hash: &str, incoming: &SyncVersion) -> Result<bool> {
        let conn = self.conn.lock();
        let current = query_record_by_hash(&conn, id_hash)?.map(|record| record.version);
        let tombstone = query_tombstone(&conn, id_hash)?;
        let newest = match (current, tombstone) {
            (Some(left), Some(right)) => Some(if left.newer_than(&right) { left } else { right }),
            (Some(version), None) | (None, Some(version)) => Some(version),
            (None, None) => None,
        };
        Ok(newest.is_none_or(|version| incoming.newer_than(&version)))
    }

    fn record_tombstone(&self, id_hash: &str, version: &SyncVersion) -> Result<()> {
        let conn = self.conn.lock();
        let existing = query_tombstone(&conn, id_hash)?;
        if existing.is_some_and(|current| !version.newer_than(&current)) {
            return Ok(());
        }
        conn.execute(
            "INSERT INTO sync_tombstones (
               id_hash, origin_device_id, origin_lamport, origin_wall_ms
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id_hash) DO UPDATE SET
               origin_device_id = excluded.origin_device_id,
               origin_lamport = excluded.origin_lamport,
               origin_wall_ms = excluded.origin_wall_ms",
            params![
                id_hash,
                version.device_id,
                to_sql_i64(version.lamport),
                version.wall_ms
            ],
        )?;
        Ok(())
    }

    fn watch_rows(&self) -> Result<HashMap<String, WatchRow>> {
        let conn = self.conn.lock();
        Self::ensure_all_records(&conn)?;
        let mut statement = conn.prepare(
            "SELECT i.id, s.id_hash, i.kind, i.content, i.hash, i.favorite,
                    COALESCE(i.file_assets, ''), COALESCE(i.image_path, ''),
                    COALESCE(i.thumb_path, '')
               FROM items i
               JOIN item_sync s ON s.item_id = i.id",
        )?;
        let rows = statement.query_map([], |row| {
            let kind = ItemKind::from_db_value(&row.get::<_, String>(2)?);
            let file_assets: String = row.get(6)?;
            let image_path: String = row.get(7)?;
            let thumb_path: String = row.get(8)?;
            Ok(WatchRow {
                item_id: row.get(0)?,
                id_hash: row.get(1)?,
                kind,
                content: row.get(3)?,
                content_hash: row.get(4)?,
                favorite: row.get::<_, i32>(5)? != 0,
                assets_fingerprint: format!("{file_assets}|{image_path}|{thumb_path}"),
            })
        })?;
        let mut output = HashMap::new();
        for row in rows {
            let row = row?;
            output.insert(row.id_hash.clone(), row);
        }
        Ok(output)
    }
}

fn query_record_by_id(conn: &Connection, item_id: i64) -> Result<Option<SyncRecord>> {
    conn.query_row(
        "SELECT s.item_id, s.id_hash, i.hash, s.origin_device_id,
                s.origin_lamport, s.origin_wall_ms
           FROM item_sync s
           JOIN items i ON i.id = s.item_id
          WHERE s.item_id = ?1",
        params![item_id],
        row_to_sync_record,
    )
    .optional()
    .map_err(Into::into)
}

fn query_record_by_hash(conn: &Connection, id_hash: &str) -> Result<Option<SyncRecord>> {
    conn.query_row(
        "SELECT s.item_id, s.id_hash, i.hash, s.origin_device_id,
                s.origin_lamport, s.origin_wall_ms
           FROM item_sync s
           JOIN items i ON i.id = s.item_id
          WHERE s.id_hash = ?1",
        params![id_hash],
        row_to_sync_record,
    )
    .optional()
    .map_err(Into::into)
}

fn row_to_sync_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<SyncRecord> {
    let lamport: i64 = row.get(4)?;
    Ok(SyncRecord {
        item_id: row.get(0)?,
        id_hash: row.get(1)?,
        content_hash: row.get(2)?,
        version: SyncVersion {
            device_id: row.get(3)?,
            lamport: lamport.max(0) as u64,
            wall_ms: row.get(5)?,
        },
    })
}

fn query_tombstone(conn: &Connection, id_hash: &str) -> Result<Option<SyncVersion>> {
    conn.query_row(
        "SELECT origin_device_id, origin_lamport, origin_wall_ms
           FROM sync_tombstones WHERE id_hash = ?1",
        params![id_hash],
        |row| {
            let lamport: i64 = row.get(1)?;
            Ok(SyncVersion {
                device_id: row.get(0)?,
                lamport: lamport.max(0) as u64,
                wall_ms: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn to_sql_i64(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_preferences_are_safe_by_default() {
        let preferences = SyncPreferences::default();
        assert!(preferences.sync_text);
        assert!(preferences.sync_images);
        assert!(!preferences.sync_files);
        assert_eq!(preferences.sync_file_mode, SyncFileMode::Blocklist);
        assert!(preferences.sync_file_extensions.contains(&".exe".into()));
        assert!(preferences.sync_file_extensions.contains(&".mp4".into()));
    }

    #[test]
    fn file_policy_supports_allowlist_blocklist_and_all() {
        let mut preferences = SyncPreferences::default();
        assert!(!preferences.allows_file_name("setup.exe"));
        assert!(preferences.allows_file_name("notes.txt"));

        preferences.sync_file_mode = SyncFileMode::Allowlist;
        preferences.sync_file_extensions = vec![".txt".into(), ".md".into()];
        assert!(preferences.allows_file_name("notes.TXT"));
        assert!(!preferences.allows_file_name("manual.pdf"));

        preferences.sync_file_mode = SyncFileMode::All;
        assert!(preferences.allows_file_name("movie.mp4"));
    }

    #[test]
    fn newer_version_uses_lamport_wall_clock_then_device_id() {
        let base = SyncVersion {
            device_id: "a".into(),
            lamport: 10,
            wall_ms: 100,
        };
        assert!(SyncVersion {
            device_id: "a".into(),
            lamport: 11,
            wall_ms: 1,
        }
        .newer_than(&base));
        assert!(SyncVersion {
            device_id: "a".into(),
            lamport: 10,
            wall_ms: 101,
        }
        .newer_than(&base));
        assert!(SyncVersion {
            device_id: "b".into(),
            lamport: 10,
            wall_ms: 100,
        }
        .newer_than(&base));
    }

    #[test]
    fn bounded_queue_drops_oldest_jobs() {
        let queue = SyncQueue::default();
        let first = SyncJob {
            body: SyncBody::Tombstone {
                id_hash: "aaaaaaaaaaaaaaaa".into(),
                version: SyncVersion {
                    device_id: "a".into(),
                    lamport: 1,
                    wall_ms: 1,
                },
            },
            blobs: Vec::new(),
            estimated_bytes: 8,
        };
        let second = SyncJob {
            body: SyncBody::Tombstone {
                id_hash: "bbbbbbbbbbbbbbbb".into(),
                version: SyncVersion {
                    device_id: "a".into(),
                    lamport: 2,
                    wall_ms: 2,
                },
            },
            blobs: Vec::new(),
            estimated_bytes: 8,
        };
        assert!(queue.push(first, 8));
        assert!(queue.push(second, 8));
        assert_eq!(queue.pop().body.id_hash(), "bbbbbbbbbbbbbbbb");
    }

    #[test]
    fn sanitize_remote_file_name_removes_path_components() {
        assert_eq!(safe_file_name(r"..\\..\\secret.txt"), "secret.txt");
        assert_eq!(safe_file_name("../../secret.txt"), "secret.txt");
    }

    #[test]
    fn rust_deserializes_android_clip_upsert_golden_json() {
        let json = r##"{
          "protocol":"clipmo-lan-v2",
          "pairingCode":"123456",
          "device":{"id":"android-device","name":"Pixel","platform":"android","color":"#78F13D"},
          "tcpPort":47634,
          "body":{
            "type":"clipUpsert",
            "clip":{
              "idHash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
              "kind":"text",
              "preview":"from Android",
              "content":"from Android",
              "contentHash":"22222222222222222222222222222222",
              "favorite":false,
              "copiedAt":1700000000000,
              "version":{"deviceId":"android-device","lamport":42,"wallMs":1700000000000}
            }
          }
        }"##;

        let envelope: SyncEnvelope = serde_json::from_str(json).expect("Android wire JSON");
        assert_eq!(envelope.pairing_code, "123456");
        assert_eq!(envelope.tcp_port, 47_634);
        match envelope.body {
            SyncBody::ClipUpsert { clip } => {
                assert_eq!(clip.content, "from Android");
                assert_eq!(clip.version.device_id, "android-device");
            }
            _ => panic!("expected clipUpsert"),
        }
    }

    #[test]
    fn rust_serializes_clip_upsert_with_android_field_names() {
        let envelope = SyncEnvelope {
            protocol: PROTOCOL.into(),
            pairing_code: "123456".into(),
            device: DeviceIdentity {
                id: "windows-device".into(),
                name: "Desktop".into(),
                platform: PlatformKind::Windows,
                color: "#1677FF".into(),
            },
            tcp_port: 47_634,
            body: SyncBody::ClipUpsert {
                clip: ClipSnapshot {
                    id_hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
                    kind: ItemKind::Text,
                    preview: "hello".into(),
                    content: "hello".into(),
                    content_hash: "33333333333333333333333333333333".into(),
                    favorite: false,
                    copied_at: 1_700_000_000_001,
                    version: SyncVersion {
                        device_id: "windows-device".into(),
                        lamport: 7,
                        wall_ms: 1_700_000_000_001,
                    },
                },
            },
        };
        let value = serde_json::to_value(envelope).expect("Rust wire JSON");
        assert_eq!(value["pairingCode"], "123456");
        assert_eq!(value["tcpPort"], 47_634);
        assert_eq!(value["body"]["type"], "clipUpsert");
        assert_eq!(
            value["body"]["clip"]["idHash"],
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );
        assert_eq!(
            value["body"]["clip"]["version"]["deviceId"],
            "windows-device"
        );
    }
}
