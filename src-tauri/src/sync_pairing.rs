//! Invitations are temporary; each confirmed connection has its own credential.
use super::*;

const PAIRING_WINDOW_MS: i64 = 120_000;

#[derive(Default)]
pub(super) struct Pairing {
    pub trust_lock: Mutex<()>,
    join_epoch: AtomicU64,
    pub until: AtomicU64,
    pub candidates: RwLock<HashMap<String, (DiscoveryMessage, SocketAddr, i64)>>,
    attempts: Mutex<(i64, u32)>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Control {
    protocol: String,
    kind: String,
    device: DeviceIdentity,
    tcp_port: u16,
    #[serde(default)]
    code: String,
    token: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ControlReply {
    ok: bool,
    device: DeviceIdentity,
    token: String,
}

pub fn new_code(previous: &str) -> String {
    loop {
        let code = format!("{:06}", uuid::Uuid::new_v4().as_u128() % 1_000_000);
        if code != previous {
            return code;
        }
    }
}

impl SyncService {
    pub(super) fn local_pairing_address(&self) -> Option<String> {
        if self.listen_port == 0 {
            return None;
        }
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
        socket.set_broadcast(true).ok()?;
        // Select the LAN route without sending a packet.
        socket.connect((Ipv4Addr::BROADCAST, DISCOVERY_PORT)).ok()?;
        let ip = socket.local_addr().ok()?.ip();
        if ip.is_unspecified() || ip.is_loopback() {
            return None;
        }
        Some(SocketAddr::new(ip, self.listen_port).to_string())
    }

    pub(super) fn compatibility_warning(&self) -> Option<String> {
        let old = self
            .pairing
            .candidates
            .read()
            .values()
            .any(|(message, _, seen)| now_ms() - *seen < 15_000 && message.protocol != PROTOCOL);
        old.then(|| "An incompatible Clipmo device is nearby. Install Clipmo 0.2.12 or newer on every device; published 0.2.11 cannot pair with this app.".into())
    }

    pub fn pairing_open(&self) -> bool {
        self.pairing.until.load(Ordering::SeqCst) > now_ms() as u64
    }

    pub fn set_pairing(&self, open: bool) {
        if open {
            *self.pairing.attempts.lock() = (now_ms(), 0);
        }
        self.pairing.until.store(
            if open {
                (now_ms() + PAIRING_WINDOW_MS) as u64
            } else {
                0
            },
            Ordering::SeqCst,
        );
    }

    fn save_peer(&self, peer: &PeerRecord) -> Result<()> {
        let store = self
            .store
            .as_ref()
            .ok_or_else(|| Error::Other("Sync is unavailable".into()))?;
        store.conn.lock().execute(
            "INSERT OR REPLACE INTO sync_preferences(key,value) VALUES(?1,?2)",
            params![
                format!("peer:{}", peer.device.id),
                serde_json::to_string(peer).map_err(io::Error::other)?
            ],
        )?;
        self.peers
            .write()
            .insert(peer.device.id.clone(), peer.clone());
        Ok(())
    }

    pub(super) fn load_peers(&self) -> Result<()> {
        if let Some(store) = &self.store {
            let conn = store.conn.lock();
            let mut stmt =
                conn.prepare("SELECT value FROM sync_preferences WHERE key LIKE 'peer:%'")?;
            let values = stmt.query_map([], |row| row.get::<_, String>(0))?;
            for value in values {
                let mut peer: PeerRecord =
                    serde_json::from_str(&value?).map_err(io::Error::other)?;
                peer.last_seen_at = 0; // A saved address is not proof of a live connection.
                self.peers.write().insert(peer.device.id.clone(), peer);
            }
        }
        Ok(())
    }

    pub fn forget_peer(&self, id: &str) -> Result<()> {
        let _guard = self.pairing.trust_lock.lock();
        self.set_pairing(false);
        self.pairing.join_epoch.fetch_add(1, Ordering::SeqCst);
        // Deleting the credential makes an offline removal effective immediately.
        if let Some(store) = &self.store {
            store.conn.lock().execute(
                "DELETE FROM sync_preferences WHERE key=?1",
                [format!("peer:{id}")],
            )?;
        }
        self.peers.write().remove(id);
        Ok(())
    }

    pub(super) fn authenticates(&self, id: &str, token: &str) -> bool {
        valid_token(token)
            && self
                .peers
                .read()
                .get(id)
                .is_some_and(|peer| peer.token == token)
    }

    pub fn join_device(
        &self,
        code: String,
        target_id: Option<String>,
        address: Option<String>,
    ) -> Result<()> {
        if code.len() != 6 || !code.bytes().all(|ch| ch.is_ascii_digit()) {
            return Err(Error::Other("Enter a six-digit code".into()));
        }
        let current = self
            .settings
            .as_ref()
            .ok_or_else(|| Error::Other("Sync unavailable".into()))?
            .read()
            .clone();
        let direct = address.filter(|s| !s.trim().is_empty()).map(|value| {
            value.trim().parse::<SocketAddr>().ok().filter(|addr| {
                matches!(addr.ip(), IpAddr::V4(ip) if ip.is_private() || ip.is_link_local())
                    && (FIRST_SYNC_PORT..=LAST_SYNC_PORT).contains(&addr.port())
            }).ok_or_else(|| Error::Other("Enter the LAN address shown on the other device, for example 192.168.1.4:47634".into()))
        }).transpose()?;
        if code == current.sync_pairing_code
            && target_id
                .as_deref()
                .is_none_or(|id| id == current.sync_device_id)
        {
            return Err(Error::Other(
                "Enter the code from the other device, not this device".into(),
            ));
        }
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let epoch = self.pairing.join_epoch.fetch_add(1, Ordering::SeqCst) + 1;
        let deadline = now_ms() + 20_000;
        let mut attempted = HashMap::new();
        let mut last_error = None;
        let mut saw_compatible = false;
        while now_ms() < deadline {
            if !self.settings.as_ref().unwrap().read().sync_enabled
                || self.pairing.join_epoch.load(Ordering::SeqCst) != epoch
            {
                break;
            }
            let mut candidates: Vec<_> = self.pairing.candidates.read().values().cloned().collect();
            // A known address remains useful when routers suppress UDP discovery.
            for peer in self.peers.read().values() {
                candidates.push((
                    DiscoveryMessage {
                        protocol: PROTOCOL.into(),
                        pairing_code: String::new(),
                        pairing_available: true,
                        device: peer.device.clone(),
                        tcp_port: peer.address.port(),
                    },
                    peer.address,
                    now_ms(),
                ));
            }
            if let Some(address) = direct {
                let mut device = current.device_identity();
                device.id = target_id.clone().unwrap_or_default();
                candidates = vec![(
                    DiscoveryMessage {
                        protocol: PROTOCOL.into(),
                        pairing_code: String::new(),
                        pairing_available: true,
                        device,
                        tcp_port: address.port(),
                    },
                    address,
                    now_ms(),
                )];
            }
            for (candidate, address, seen) in candidates {
                if now_ms() >= deadline || self.pairing.join_epoch.load(Ordering::SeqCst) != epoch {
                    break;
                }
                if candidate.protocol != PROTOCOL {
                    continue;
                }
                if now_ms() - seen <= 10_000 {
                    saw_compatible = true;
                }
                if now_ms() - seen > 10_000
                    || !candidate.pairing_available
                    || target_id
                        .as_ref()
                        .is_some_and(|id| *id != candidate.device.id)
                    || attempted
                        .get(&candidate.device.id)
                        .is_some_and(|last| now_ms() - last < 3_000)
                {
                    continue;
                }
                attempted.insert(candidate.device.id.clone(), now_ms());
                match self.exchange_control(address, &candidate.device.id, "pair", &code, &token) {
                    Ok(()) => {
                        self.enqueue_history_backfill();
                        return Ok(());
                    }
                    Err(error) => last_error = Some(error.kind()),
                }
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        let message = match last_error {
            Some(io::ErrorKind::PermissionDenied) => "The other device rejected this invitation. Choose New code on that device and enter it here before it expires.",
            Some(_) => "A compatible device was found but did not complete the connection. Allow incoming Clipmo connections through its firewall and try a fresh code.",
            None if !saw_compatible && self.compatibility_warning().is_some() => "Only an incompatible Clipmo device was found. Install Clipmo 0.2.12 or newer on every device. Published 0.2.11 uses an incompatible pairing protocol.",
            None if saw_compatible => "A compatible device was found but pairing is closed. Choose New code on that device, then enter its code here.",
            None => "No compatible device was discovered. Keep both apps open on the same Wi-Fi, enable LAN sync, and choose New code on the other device. Android can scan the QR code to connect directly.",
        };
        Err(Error::Other(message.into()))
    }

    pub(super) fn exchange_control(
        &self,
        address: SocketAddr,
        id: &str,
        kind: &str,
        code: &str,
        token: &str,
    ) -> io::Result<()> {
        let current = self.settings.as_ref().unwrap().read().clone();
        let epoch = self.pairing.join_epoch.load(Ordering::SeqCst);
        let request = Control {
            protocol: PROTOCOL.into(),
            kind: kind.into(),
            device: current.device_identity(),
            tcp_port: self.listen_port,
            code: code.into(),
            token: token.into(),
        };
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(3))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        write_json(&mut stream, &request)?;
        let reply: ControlReply = read_json(&mut stream)?;
        if !reply.ok
            || (!id.is_empty() && reply.device.id != id)
            || reply.device.id == current.sync_device_id
            || reply.token != token
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Pairing or device authentication failed",
            ));
        }
        let _guard = self.pairing.trust_lock.lock();
        if self.pairing.join_epoch.load(Ordering::SeqCst) != epoch
            || !self.settings.as_ref().unwrap().read().sync_enabled
        {
            return Err(io::Error::other("Connection attempt cancelled"));
        }
        if kind != "pair" && !self.authenticates(id, token) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Device was removed",
            ));
        }
        let peer = PeerRecord {
            device: reply.device,
            address,
            last_seen_at: now_ms(),
            token: token.into(),
        };
        self.save_peer(&peer)
            .map_err(|e| io::Error::other(e.to_string()))
    }

    pub(super) fn handle_control(
        &self,
        value: serde_json::Value,
        stream: &mut TcpStream,
        source: SocketAddr,
    ) -> io::Result<()> {
        let _guard = self.pairing.trust_lock.lock();
        let request: Control = serde_json::from_value(value).map_err(io::Error::other)?;
        let current = self.settings.as_ref().unwrap().read().clone();
        let valid = current.sync_enabled
            && request.protocol == PROTOCOL
            && request.device.id != current.sync_device_id
            && request.tcp_port != 0
            && valid_token(&request.token);
        let authenticated = self.authenticates(&request.device.id, &request.token);
        let accepted = if request.kind == "pair" {
            let mut attempts = self.pairing.attempts.lock();
            if now_ms() - attempts.0 > 60_000 {
                *attempts = (now_ms(), 0);
            }
            attempts.1 += 1;
            valid
                && self.pairing_open()
                && attempts.1 <= 20
                && request.code == current.sync_pairing_code
        } else {
            valid && request.kind == "ping" && authenticated
        };
        if accepted {
            self.save_peer(&PeerRecord {
                device: request.device.clone(),
                address: SocketAddr::new(source.ip(), request.tcp_port),
                last_seen_at: now_ms(),
                token: request.token.clone(),
            })
            .map_err(|e| io::Error::other(e.to_string()))?;
        }
        write_json(
            stream,
            &ControlReply {
                ok: accepted,
                device: current.device_identity(),
                token: if accepted {
                    request.token
                } else {
                    String::new()
                },
            },
        )?;
        // Only backfill after the response is flushed, so both sides can persist trust first.
        if accepted && request.kind == "pair" {
            self.enqueue_history_backfill();
        }
        Ok(())
    }
}

fn valid_token(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|c| c.is_ascii_hexdigit())
}

fn write_json(stream: &mut TcpStream, value: &impl Serialize) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()
}

pub(super) fn read_json<T: serde::de::DeserializeOwned>(stream: &mut TcpStream) -> io::Result<T> {
    let mut length = [0u8; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > MAX_HEADER_BYTES {
        return Err(io::Error::other("Invalid frame size"));
    }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incompatible_discovery_is_diagnostic_not_trust_and_expires() {
        let service = device("mac");
        let message = DiscoveryMessage {
            protocol: "clipmo-lan-v2".into(),
            pairing_code: "123456".into(),
            pairing_available: false,
            device: Settings::default().device_identity(),
            tcp_port: FIRST_SYNC_PORT,
        };
        service.pairing.candidates.write().insert(
            "old".into(),
            (message, "192.168.1.2:47634".parse().unwrap(), now_ms()),
        );
        assert!(service.compatibility_warning().unwrap().contains("0.2.11"));
        assert!(service.peers.read().is_empty());
        service.pairing.candidates.write().get_mut("old").unwrap().2 -= 16_000;
        assert!(service.compatibility_warning().is_none());
    }

    fn device(id: &str) -> SyncService {
        let mut service = SyncService::inactive();
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sync_preferences(key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        )
        .unwrap();
        service.store = Some(Arc::new(SyncStore {
            conn: Mutex::new(conn),
        }));
        service.settings = Some(Arc::new(RwLock::new(Settings {
            sync_device_id: id.into(),
            sync_enabled: true,
            sync_pairing_code: new_code(""),
            ..Settings::default()
        })));
        service.listen_port = FIRST_SYNC_PORT;
        service
    }

    #[test]
    fn repairing_a_saved_device_works_without_udp_discovery() {
        let client = device("client");
        let host = device("host");
        host.set_pairing(true);
        let code = host
            .settings
            .as_ref()
            .unwrap()
            .read()
            .sync_pairing_code
            .clone();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        client
            .save_peer(&PeerRecord {
                device: host.settings.as_ref().unwrap().read().device_identity(),
                address,
                last_seen_at: 0,
                token: "a".repeat(64),
            })
            .unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, source) = listener.accept().unwrap();
            let request = read_json(&mut stream).unwrap();
            host.handle_control(request, &mut stream, source).unwrap();
        });
        assert!(client.pairing.candidates.read().is_empty());
        client.join_device(code, None, None).unwrap();
        worker.join().unwrap();
        assert_ne!(client.peers.read()["host"].token, "a".repeat(64));
    }

    // Real framed TCP exchanges exercise both request/response and durable trust.
    fn exchange(
        client: &SyncService,
        host: &SyncService,
        kind: &str,
        code: &str,
        token: &str,
    ) -> bool {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = host.clone();
        let worker = std::thread::spawn(move || {
            let (mut socket, source) = listener.accept().unwrap();
            let value = read_json(&mut socket).unwrap();
            server.handle_control(value, &mut socket, source).unwrap();
        });
        let id = host
            .settings
            .as_ref()
            .unwrap()
            .read()
            .sync_device_id
            .clone();
        let ok = client
            .exchange_control(address, &id, kind, code, token)
            .is_ok();
        worker.join().unwrap();
        ok
    }

    #[test]
    fn three_devices_keep_independent_credentials_across_code_rotation_and_restart() {
        let mac = device("mac");
        let windows = device("windows");
        let android = device("android");
        mac.set_pairing(true);
        let old_code = mac
            .settings
            .as_ref()
            .unwrap()
            .read()
            .sync_pairing_code
            .clone();
        assert!(exchange(&windows, &mac, "pair", &old_code, &"a".repeat(64)));
        let new_code = new_code(&old_code);
        mac.settings.as_ref().unwrap().write().sync_pairing_code = new_code.clone();
        assert!(exchange(&android, &mac, "pair", &new_code, &"b".repeat(64)));
        mac.set_pairing(false);
        mac.peers.write().clear();
        mac.load_peers().unwrap();
        assert_eq!(mac.peers.read().len(), 2);
        assert!(mac.peers.read().values().all(|peer| peer.last_seen_at == 0));
        assert!(exchange(&windows, &mac, "ping", "", &"a".repeat(64)));
        assert!(exchange(&android, &mac, "ping", "", &"b".repeat(64)));
        assert!(!exchange(&android, &mac, "ping", "", &"a".repeat(64)));
        assert_ne!(
            windows.settings.as_ref().unwrap().read().sync_pairing_code,
            old_code
        );
    }

    #[test]
    fn closed_expired_wrong_and_rotated_codes_do_not_create_connections() {
        let host = device("host");
        let client = device("client");
        let code = host
            .settings
            .as_ref()
            .unwrap()
            .read()
            .sync_pairing_code
            .clone();
        let token = "c".repeat(64);
        assert!(!exchange(&client, &host, "pair", &code, &token));
        host.set_pairing(true);
        assert!(!exchange(&client, &host, "pair", &new_code(&code), &token));
        host.pairing
            .until
            .store((now_ms() - 1) as u64, Ordering::SeqCst);
        assert!(!exchange(&client, &host, "pair", &code, &token));
        assert!(host.peers.read().is_empty());
        assert!(client.peers.read().is_empty());
    }

    #[test]
    fn removal_revokes_one_device_durably_without_affecting_others() {
        let host = device("host");
        let first = device("first");
        let second = device("second");
        host.set_pairing(true);
        let code = host
            .settings
            .as_ref()
            .unwrap()
            .read()
            .sync_pairing_code
            .clone();
        assert!(exchange(&first, &host, "pair", &code, &"d".repeat(64)));
        assert!(exchange(&second, &host, "pair", &code, &"e".repeat(64)));
        host.forget_peer("first").unwrap();
        host.peers.write().clear();
        host.load_peers().unwrap();
        assert!(!exchange(&first, &host, "ping", "", &"d".repeat(64)));
        assert!(exchange(&second, &host, "ping", "", &"e".repeat(64)));
        assert!(!host.authenticates("first", &"d".repeat(64)));
    }

    #[test]
    fn invitations_never_advertise_a_secret() {
        let host = device("host");
        let discovery = DiscoveryMessage {
            protocol: PROTOCOL.into(),
            pairing_code: String::new(),
            pairing_available: true,
            device: host.settings.as_ref().unwrap().read().device_identity(),
            tcp_port: FIRST_SYNC_PORT,
        };
        let value = serde_json::to_value(discovery).unwrap();
        assert_eq!(value["pairingCode"], "");
        assert_eq!(value["pairingAvailable"], true);
        assert!(!valid_token("123456"));
        assert!(!valid_token(&"x".repeat(64)));
    }

    #[test]
    fn sync_delivery_requires_the_receivers_acknowledgement() {
        for accepted in [true, false] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let worker = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let envelope: SyncEnvelope = read_json(&mut stream).unwrap();
                assert_eq!(envelope.protocol, PROTOCOL);
                write_sync_ack(&mut stream, accepted, None).unwrap();
            });
            let host = device("host");
            let envelope = SyncEnvelope {
                protocol: PROTOCOL.into(),
                pairing_code: "a".repeat(64),
                device: host.settings.as_ref().unwrap().read().device_identity(),
                tcp_port: FIRST_SYNC_PORT,
                body: SyncBody::FavoriteToggle {
                    id_hash: "b".repeat(32),
                    favorite: true,
                    version: SyncVersion {
                        device_id: "host".into(),
                        lamport: 1,
                        wall_ms: 1,
                    },
                },
            };
            assert_eq!(send_frame(address, &envelope, &[]).is_ok(), accepted);
            worker.join().unwrap();
        }
    }
}
