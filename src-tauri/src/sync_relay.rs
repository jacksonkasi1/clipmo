//! Optional end-to-end-encrypted relay for networks where direct LAN sync is blocked
//! (client isolation, firewalls, different networks).
//!
//! The relay is a dumb mailbox that the *user* owns: their own HTTP endpoint (for
//! example the Cloudflare Worker in `relay/`) or their own Firebase Realtime
//! Database. Clipmo's developer never sees or stores anything. Payloads are
//! encrypted with AES-256-GCM using a key derived from a passphrase that never
//! leaves the user's devices, so the mailbox only holds ciphertext.
//!
//! Only text-like clips, edits, favorites and deletes travel over the relay; images
//! and files stay on the direct LAN path.
use super::*;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use std::collections::VecDeque;

const KDF_ROUNDS: u32 = 200_000;
const KDF_SALT: &[u8] = b"clipmo-relay-v1";
const POLL_INTERVAL: Duration = Duration::from_secs(4);
const MAX_PLAINTEXT: usize = 512 * 1024;
const PREFERENCE_KEY: &str = "relay";
const CURSOR_KEY: &str = "relay_cursor";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RelayConfig {
    pub enabled: bool,
    /// `endpoint` (any HTTPS service speaking the Clipmo relay API) or `firebase`.
    pub backend: String,
    pub url: String,
    /// Bearer token for `endpoint`, database secret / auth token for `firebase`.
    pub secret: String,
    /// Hex AES key derived from the passphrase. Never sent anywhere.
    pub key: String,
    /// Mailbox name derived from the key; reveals nothing about it.
    pub channel: String,
}

/// What the UI may see: never the secret or key.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayStatus {
    pub enabled: bool,
    pub backend: String,
    pub url: String,
    pub has_secret: bool,
    pub has_passphrase: bool,
    pub last_ok_at: i64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayInput {
    pub enabled: bool,
    pub backend: String,
    pub url: String,
    /// `None` keeps the stored secret.
    pub secret: Option<String>,
    /// `None` keeps the stored key.
    pub passphrase: Option<String>,
}

#[derive(Default)]
pub(super) struct Relay {
    config: RwLock<RelayConfig>,
    outbox: Mutex<VecDeque<(String, String)>>,
    last_ok_at: AtomicU64,
    last_error: RwLock<Option<String>>,
}

#[derive(Serialize, Deserialize)]
struct MailItem {
    id: String,
    device: String,
    blob: String,
}

pub fn derive_key(passphrase: &str) -> ([u8; 32], String) {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(passphrase.as_bytes(), KDF_SALT, KDF_ROUNDS, &mut key);
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(key);
    hasher.update(b"clipmo-relay-channel");
    let channel = hex::encode(&hasher.finalize()[..16]);
    (key, channel)
}

fn encrypt(key_hex: &str, plaintext: &[u8]) -> io::Result<String> {
    let key = hex::decode(key_hex).map_err(io::Error::other)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(io::Error::other)?;
    let random = uuid::Uuid::new_v4();
    let nonce_bytes = &random.as_bytes()[..12];
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(nonce_bytes), plaintext)
        .map_err(|_| io::Error::other("encryption failed"))?;
    let mut out = nonce_bytes.to_vec();
    out.extend(ciphertext);
    Ok(B64.encode(out))
}

fn decrypt(key_hex: &str, blob: &str) -> io::Result<Vec<u8>> {
    let key = hex::decode(key_hex).map_err(io::Error::other)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(io::Error::other)?;
    let bytes = B64.decode(blob).map_err(io::Error::other)?;
    if bytes.len() < 13 {
        return Err(io::Error::other("relay message too short"));
    }
    let (nonce, ciphertext) = bytes.split_at(12);
    cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| io::Error::other("relay message could not be decrypted (wrong passphrase?)"))
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .build()
}

fn http_error(error: ureq::Error) -> io::Error {
    match error {
        ureq::Error::Status(code, _) => io::Error::other(format!("relay returned HTTP {code}")),
        other => io::Error::other(format!("relay unreachable: {other}")),
    }
}

fn push_item(config: &RelayConfig, item: &MailItem) -> io::Result<()> {
    let base = config.url.trim_end_matches('/');
    match config.backend.as_str() {
        "firebase" => {
            let url = format!(
                "{base}/clipmo/{}/{}.json?auth={}",
                config.channel,
                item.id,
                url::form_urlencoded::byte_serialize(config.secret.as_bytes()).collect::<String>()
            );
            agent()
                .put(&url)
                .send_json(serde_json::json!({ "d": item.device, "b": item.blob }))
                .map_err(http_error)?;
        }
        _ => {
            agent()
                .post(&format!("{base}/v1/{}", config.channel))
                .set("Authorization", &format!("Bearer {}", config.secret))
                .send_json(item)
                .map_err(http_error)?;
        }
    }
    Ok(())
}

fn pull_items(config: &RelayConfig, after: &str) -> io::Result<Vec<MailItem>> {
    let base = config.url.trim_end_matches('/');
    let mut items: Vec<MailItem> = match config.backend.as_str() {
        "firebase" => {
            let enc = |value: &str| {
                url::form_urlencoded::byte_serialize(value.as_bytes()).collect::<String>()
            };
            let url = format!(
                "{base}/clipmo/{}.json?auth={}&orderBy={}&startAt={}&limitToFirst=200",
                config.channel,
                enc(&config.secret),
                enc("\"$key\""),
                enc(&format!("\"{after}\"")),
            );
            let value: serde_json::Value = agent()
                .get(&url)
                .call()
                .map_err(http_error)?
                .into_json()
                .map_err(io::Error::other)?;
            value
                .as_object()
                .map(|map| {
                    map.iter()
                        .filter_map(|(id, entry)| {
                            Some(MailItem {
                                id: id.clone(),
                                device: entry.get("d")?.as_str()?.to_string(),
                                blob: entry.get("b")?.as_str()?.to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
        _ => {
            #[derive(Deserialize)]
            struct Page {
                items: Vec<MailItem>,
            }
            let page: Page = agent()
                .get(&format!("{base}/v1/{}", config.channel))
                .set("Authorization", &format!("Bearer {}", config.secret))
                .query("after", after)
                .call()
                .map_err(http_error)?
                .into_json()
                .map_err(io::Error::other)?;
            page.items
        }
    };
    items.sort_by(|a, b| a.id.cmp(&b.id));
    items.retain(|item| item.id.as_str() > after);
    Ok(items)
}

/// Sortable, unique, and parseable without decrypting: `<ms>-<random>`.
fn new_item_id() -> String {
    format!(
        "{:016}-{}",
        now_ms().max(0),
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    )
}

impl SyncService {
    pub fn relay_status(&self) -> RelayStatus {
        let config = self.relay.config.read().clone();
        RelayStatus {
            enabled: config.enabled,
            backend: config.backend.clone(),
            url: config.url.clone(),
            has_secret: !config.secret.is_empty(),
            has_passphrase: !config.key.is_empty(),
            last_ok_at: self.relay.last_ok_at.load(Ordering::SeqCst) as i64,
            last_error: self.relay.last_error.read().clone(),
        }
    }

    pub fn save_relay(&self, input: RelayInput) -> Result<RelayStatus> {
        let mut config = self.relay.config.read().clone();
        config.enabled = input.enabled;
        config.backend = if input.backend == "firebase" {
            "firebase"
        } else {
            "endpoint"
        }
        .into();
        config.url = input.url.trim().to_string();
        if let Some(secret) = input.secret {
            config.secret = secret.trim().to_string();
        }
        if let Some(passphrase) = input.passphrase.filter(|value| !value.is_empty()) {
            let (key, channel) = derive_key(&passphrase);
            config.key = hex::encode(key);
            config.channel = channel;
        }
        if config.enabled {
            if !(config.url.starts_with("https://")
                || config.url.starts_with("http://localhost")
                || config.url.starts_with("http://127.0.0.1"))
            {
                return Err(Error::Other(
                    "The relay address must start with https://".into(),
                ));
            }
            if config.key.is_empty() {
                return Err(Error::Other(
                    "Choose a sync passphrase for the relay.".into(),
                ));
            }
        }
        if let Some(store) = &self.store {
            let json = serde_json::to_string(&config).map_err(|e| Error::Other(e.to_string()))?;
            store.conn.lock().execute(
                "INSERT INTO sync_preferences(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![PREFERENCE_KEY, json],
            )?;
        }
        *self.relay.config.write() = config;
        *self.relay.last_error.write() = None;
        Ok(self.relay_status())
    }

    pub(super) fn load_relay(&self) {
        let Some(store) = &self.store else { return };
        let raw: Option<String> = store
            .conn
            .lock()
            .query_row(
                "SELECT value FROM sync_preferences WHERE key = ?1",
                params![PREFERENCE_KEY],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten();
        if let Some(config) = raw.and_then(|json| serde_json::from_str::<RelayConfig>(&json).ok()) {
            *self.relay.config.write() = config;
        }
    }

    /// Queues an already-built envelope for the relay if it is relay-eligible.
    pub(super) fn relay_enqueue(&self, envelope: &SyncEnvelope) {
        if !self.relay.config.read().enabled {
            return;
        }
        if !matches!(
            envelope.body,
            SyncBody::ClipUpsert { .. }
                | SyncBody::ClipEdit { .. }
                | SyncBody::FavoriteToggle { .. }
                | SyncBody::Tombstone { .. }
        ) {
            return;
        }
        let mut stripped = envelope.clone();
        stripped.pairing_code = String::new(); // LAN credentials never go to the relay.
        let Ok(json) = serde_json::to_string(&stripped) else {
            return;
        };
        let mut outbox = self.relay.outbox.lock();
        if outbox.len() >= 500 {
            outbox.pop_front();
        }
        outbox.push_back((new_item_id(), json));
    }

    fn relay_cursor(&self) -> String {
        let stored: Option<String> = self.store.as_ref().and_then(|store| {
            store
                .conn
                .lock()
                .query_row(
                    "SELECT value FROM sync_preferences WHERE key = ?1",
                    params![CURSOR_KEY],
                    |row| row.get(0),
                )
                .optional()
                .ok()
                .flatten()
        });
        // First run only looks at the last ten minutes, like live LAN sync.
        stored.unwrap_or_else(|| format!("{:016}-", (now_ms() - 600_000).max(0)))
    }

    fn save_relay_cursor(&self, cursor: &str) {
        if let Some(store) = &self.store {
            let _ = store.conn.lock().execute(
                "INSERT INTO sync_preferences(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![CURSOR_KEY, cursor],
            );
        }
    }

    fn relay_tick(&self, app: &AppHandle) -> io::Result<()> {
        let config = self.relay.config.read().clone();
        let Some(settings) = &self.settings else {
            return Ok(());
        };
        let own_id = settings.read().sync_device_id.clone();
        // Push.
        loop {
            let next = self.relay.outbox.lock().front().cloned();
            let Some((id, json)) = next else { break };
            let blob = encrypt(&config.key, json.as_bytes())?;
            push_item(
                &config,
                &MailItem {
                    id,
                    device: own_id.clone(),
                    blob,
                },
            )?;
            self.relay.outbox.lock().pop_front();
        }
        // Pull.
        let mut cursor = self.relay_cursor();
        let items = pull_items(&config, &cursor)?;
        let mut changed = false;
        for item in items {
            cursor = item.id.clone();
            if item.device == own_id {
                continue;
            }
            let Ok(plain) = decrypt(&config.key, &item.blob) else {
                continue;
            };
            if plain.len() > MAX_PLAINTEXT {
                continue;
            }
            let Ok(envelope) = serde_json::from_slice::<SyncEnvelope>(&plain) else {
                continue;
            };
            if envelope.device.id != item.device
                || envelope.body.version().device_id != envelope.device.id
                || !valid_id_hash(envelope.body.id_hash())
            {
                continue;
            }
            self.note_remote_version(envelope.body.version());
            match apply_incoming(&mut io::empty(), self, app, envelope) {
                Ok(()) => changed = true,
                Err(error) => log::debug!("relay message not applied: {error}"),
            }
        }
        self.save_relay_cursor(&cursor);
        self.relay
            .last_ok_at
            .store(now_ms().max(0) as u64, Ordering::SeqCst);
        if changed {
            let _ = app.emit("clip-updated", ());
        }
        Ok(())
    }
}

pub(super) fn spawn_relay_worker(service: SyncService, app: AppHandle) -> io::Result<()> {
    std::thread::Builder::new()
        .name("sync-relay".into())
        .spawn(move || loop {
            std::thread::sleep(POLL_INTERVAL);
            let enabled = service.relay.config.read().enabled
                && service
                    .settings
                    .as_ref()
                    .is_some_and(|s| s.read().sync_enabled);
            if !enabled {
                continue;
            }
            match service.relay_tick(&app) {
                Ok(()) => *service.relay.last_error.write() = None,
                Err(error) => {
                    log::debug!("relay sync failed: {error}");
                    *service.relay.last_error.write() = Some(error.to_string());
                }
            }
            let _ = app.emit("sync-peers-updated", ());
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encryption_round_trips_and_rejects_other_passphrases() {
        let (key, channel) = derive_key("correct horse");
        let (other, other_channel) = derive_key("battery staple");
        assert_ne!(channel, other_channel);
        let blob = encrypt(&hex::encode(key), b"hello clip").unwrap();
        assert_eq!(decrypt(&hex::encode(key), &blob).unwrap(), b"hello clip");
        assert!(decrypt(&hex::encode(other), &blob).is_err());
        // Two encryptions of the same text differ (random nonce).
        assert_ne!(blob, encrypt(&hex::encode(key), b"hello clip").unwrap());
    }

    #[test]
    fn key_derivation_matches_the_published_vector() {
        // Computed independently with Python's hashlib; Android must produce the same.
        let (key, channel) = derive_key("correct horse");
        assert_eq!(
            hex::encode(key),
            "baac6e243d663fff81a458081fdae391e5c62f7862bac5b88736684a21c1afff"
        );
        assert_eq!(channel, "b60c7f2e01768c60ac9353abff15c99f");
    }

    #[test]
    fn item_ids_sort_by_time() {
        let a = new_item_id();
        std::thread::sleep(Duration::from_millis(2));
        let b = new_item_id();
        assert!(a < b);
    }
}
