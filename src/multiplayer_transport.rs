// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

//! One-to-one Godot packet transport over KyProto's reliable data endpoint.
use kymux_types::DataPacket;
use kyproto::Connection;
use sha2::{Digest, Sha256};
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use tokio::{
    sync::{mpsc, oneshot},
    time::timeout,
};

pub const MAX_PACKET: usize = 65_536;
pub const QUEUE_SIZE: usize = 128;
const HELLO: &[u8] = b"godot-kyber-multiplayer/1";
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub enum Event {
    Listening(String),
    Connected,
    Packet(Vec<u8>),
    Disconnected,
    Failed(String),
}
pub enum Mode {
    Server(SocketAddr),
    Client {
        host: String,
        port: u16,
        code: String,
    },
}
pub struct Session {
    pub outgoing: mpsc::Sender<Vec<u8>>,
    pub incoming: mpsc::Receiver<Event>,
    pub refuse: Arc<AtomicBool>,
    cancel: Option<oneshot::Sender<()>>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Session {
    pub fn start(mode: Mode) -> Result<Self> {
        if let Mode::Client { code, .. } = &mode {
            parse_code(code)?;
        }
        let (outgoing, packets) = mpsc::channel(QUEUE_SIZE);
        let (events, incoming) = mpsc::channel(QUEUE_SIZE);
        let (cancel, stopped) = oneshot::channel();
        let refuse = Arc::new(AtomicBool::new(false));
        let refused = refuse.clone();
        let worker = thread::Builder::new()
            .name("kyber-multiplayer".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                match runtime {
                    Ok(runtime) => runtime.block_on(async {
                        if let Err(error) = run(mode, packets, &events, stopped, refused).await {
                            let _ = events.try_send(Event::Failed(error.to_string()));
                        }
                    }),
                    Err(error) => {
                        let _ = events.try_send(Event::Failed(error.to_string()));
                    }
                }
            })?;
        Ok(Self {
            outgoing,
            incoming,
            refuse,
            cancel: Some(cancel),
            worker: Some(worker),
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn parse_code(code: &str) -> Result<(&str, &str)> {
    let (hash, token) = code.split_once(':').ok_or("Invalid connection code")?;
    if [hash, token]
        .iter()
        .any(|s| s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()))
    {
        return Err("Invalid connection code".into());
    }
    Ok((hash, token))
}

fn transport_config() -> Arc<quinn::TransportConfig> {
    let mut config = quinn::TransportConfig::default();
    config.max_idle_timeout(Some(
        Duration::from_secs(5).try_into().expect("valid timeout"),
    ));
    config.keep_alive_interval(Some(Duration::from_secs(1)));
    Arc::new(config)
}

async fn run(
    mode: Mode,
    mut packets: mpsc::Receiver<Vec<u8>>,
    events: &mpsc::Sender<Event>,
    mut stopped: oneshot::Receiver<()>,
    refuse: Arc<AtomicBool>,
) -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (endpoint, client_options) = match mode {
        Mode::Server(address) => {
            let cert = rcgen::generate_simple_self_signed(vec!["godot-kyber.local".into()])?;
            let key = cert.signing_key.serialize_der();
            let token = hex::encode(Sha256::digest(&key));
            let code = format!("{}:{token}", hex::encode(Sha256::digest(cert.cert.der())));
            let mut tls = rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(
                    vec![cert.cert.der().clone()],
                    rustls::pki_types::PrivatePkcs8KeyDer::from(key).into(),
                )?;
            tls.alpn_protocols = vec![b"godot-kyber-rpc/1".to_vec()];
            let mut config = quinn::ServerConfig::with_crypto(Arc::new(
                quinn::crypto::rustls::QuicServerConfig::try_from(tls)?,
            ));
            config.transport_config(transport_config());
            let endpoint = quinn::Endpoint::server(config, address)?;
            events.send(Event::Listening(code)).await?;
            (endpoint, (None, token))
        }
        Mode::Client { host, port, code } => {
            let (hash, token) = parse_code(&code)?;
            let address = tokio::select! {
                _ = &mut stopped => return Ok(()),
                result = timeout(Duration::from_secs(5), tokio::net::lookup_host((host.as_str(), port))) => result??.min_by_key(|address| address.is_ipv6()).ok_or("Host not found")?,
            };
            let mut tls = rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(
                    kymux_util::cert::HashCertVerifier::new_from_hex(hash)?,
                ))
                .with_no_client_auth();
            tls.alpn_protocols = vec![b"godot-kyber-rpc/1".to_vec()];
            let mut config = quinn::ClientConfig::new(Arc::new(
                quinn::crypto::rustls::QuicClientConfig::try_from(tls)?,
            ));
            config.transport_config(transport_config());
            let mut endpoint = quinn::Endpoint::client(
                if address.is_ipv4() {
                    "0.0.0.0:0"
                } else {
                    "[::]:0"
                }
                .parse()?,
            )?;
            endpoint.set_default_client_config(config);
            (endpoint, (Some(address), token.to_owned()))
        }
    };
    let (address, token) = client_options;
    let result: Result<()> = tokio::select! {
        _ = &mut stopped => Ok(()),
        result = async {
            let connection = if let Some(address) = address {
                let transport = timeout(Duration::from_secs(5), endpoint.connect(address, "godot-kyber.local")?).await??;
                timeout(Duration::from_secs(5), Connection::connect_with_auth(transport.into(), &kyproto::ClientAuth::new(&token)?)).await??
            } else {
                loop {
                    let incoming = endpoint.accept().await.ok_or("Listener closed")?;
                    if refuse.load(Ordering::Relaxed) { incoming.refuse(); continue; }
                    let transport = match timeout(Duration::from_secs(5), incoming).await { Ok(Ok(value)) => value, _ => continue };
                    match timeout(Duration::from_secs(5), Connection::accept_with_auth(transport.into())).await {
                        Ok(Ok(pending)) if pending.get_auth().token() == token && !refuse.load(Ordering::Relaxed) => break pending.accept_authentication().await?,
                        Ok(Ok(pending)) => pending.reject_authentication(),
                        _ => {}
                    }
                }
            };
            let channel = if address.is_some() { connection.connect_data_endpoint(0)? } else { connection.register_data_endpoint().await?.1 };
            let (mut output, mut input) = timeout(Duration::from_secs(5), channel.ready()).await??;
            output.send(DataPacket { payload: HELLO.to_vec().into() }).await?;
            let hello = timeout(Duration::from_secs(5), input.recv()).await??.ok_or("Missing protocol handshake")?;
            if hello.payload.as_ref() != HELLO { return Err("Incompatible multiplayer protocol".into()); }
            events.send(Event::Connected).await?;
            // Separate send/receive tasks avoid cancellation of partially read packets.
            let send = async {
                while let Some(packet) = packets.recv().await {
                    output.send(DataPacket { payload: packet.into() }).await?;
                }
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            };
            let receive = async {
                while let Some(packet) = input.recv().await? {
                    if packet.payload.len() > MAX_PACKET { return Err("Packet exceeds maximum size".into()); }
                    events.send(Event::Packet(packet.payload.to_vec())).await?;
                }
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            };
            tokio::select! { result = send => result, result = receive => result, result = connection.closed() => result.map_err(Into::into) }
        } => result,
    };
    endpoint.close(0u32.into(), b"Closed by Godot peer");
    let _ = timeout(Duration::from_secs(1), endpoint.wait_idle()).await;
    if result.is_ok() {
        let _ = events.try_send(Event::Disconnected);
    }
    result
}
