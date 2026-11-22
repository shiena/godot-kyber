// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

use crate::multiplayer_transport::{Event, MAX_PACKET, Mode, QUEUE_SIZE, Session};
use godot::{
    classes::{
        IMultiplayerPeerExtension, MultiplayerPeerExtension,
        multiplayer_peer::{ConnectionStatus, TransferMode},
    },
    global::Error,
    prelude::*,
};
use std::{collections::VecDeque, sync::atomic::Ordering};

#[derive(GodotClass)]
#[class(tool, base = MultiplayerPeerExtension)]
struct KyberMultiplayerPeer {
    base: Base<MultiplayerPeerExtension>,
    session: Option<Session>,
    packets: VecDeque<Vec<u8>>,
    server: bool,
    connected: bool,
    status: ConnectionStatus,
    channel: i32,
    mode: TransferMode,
    target: i32,
    refuse: bool,
    code: String,
    error: String,
    terminal: bool,
}

#[godot_api]
impl IMultiplayerPeerExtension for KyberMultiplayerPeer {
    fn init(base: Base<MultiplayerPeerExtension>) -> Self {
        Self {
            base,
            session: None,
            packets: VecDeque::new(),
            server: false,
            connected: false,
            status: ConnectionStatus::DISCONNECTED,
            channel: 0,
            mode: TransferMode::RELIABLE,
            target: 0,
            refuse: false,
            code: String::new(),
            error: String::new(),
            terminal: false,
        }
    }
    fn get_available_packet_count(&self) -> i32 {
        self.packets.len() as i32
    }
    fn get_max_packet_size(&self) -> i32 {
        MAX_PACKET as i32
    }
    fn get_packet_script(&mut self) -> PackedByteArray {
        self.packets
            .pop_front()
            .map(|packet| PackedByteArray::from(packet.as_slice()))
            .unwrap_or_default()
    }
    fn put_packet_script(&mut self, packet: PackedByteArray) -> Error {
        if !self.connected {
            return Error::ERR_UNCONFIGURED;
        }
        if self.mode != TransferMode::RELIABLE || self.channel != 0 {
            return Error::ERR_UNAVAILABLE;
        }
        if packet.len() > MAX_PACKET {
            return Error::ERR_INVALID_PARAMETER;
        }
        let remote = self.remote_id();
        if self.target == -remote {
            return Error::OK;
        }
        if self.target > 0 && self.target != remote {
            return Error::ERR_INVALID_PARAMETER;
        }
        match self
            .session
            .as_ref()
            .expect("connected session")
            .outgoing
            .try_send(packet.to_vec())
        {
            Ok(()) => Error::OK,
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => Error::ERR_OUT_OF_MEMORY,
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => Error::ERR_CONNECTION_ERROR,
        }
    }
    fn get_packet_channel(&self) -> i32 {
        0
    }
    fn get_packet_mode(&self) -> TransferMode {
        TransferMode::RELIABLE
    }
    fn set_transfer_channel(&mut self, channel: i32) {
        self.channel = channel;
    }
    fn get_transfer_channel(&self) -> i32 {
        self.channel
    }
    fn set_transfer_mode(&mut self, mode: TransferMode) {
        self.mode = mode;
    }
    fn get_transfer_mode(&self) -> TransferMode {
        self.mode
    }
    fn set_target_peer(&mut self, peer: i32) {
        self.target = peer;
    }
    fn get_packet_peer(&self) -> i32 {
        self.remote_id()
    }
    fn is_server(&self) -> bool {
        self.server
    }
    fn get_unique_id(&self) -> i32 {
        if self.server { 1 } else { 2 }
    }
    fn get_connection_status(&self) -> ConnectionStatus {
        self.status
    }
    fn is_server_relay_supported(&self) -> bool {
        false
    }
    fn set_refuse_new_connections(&mut self, refuse: bool) {
        self.refuse = refuse;
        if let Some(session) = &self.session {
            session.refuse.store(refuse, Ordering::Relaxed);
        }
    }
    fn is_refusing_new_connections(&self) -> bool {
        self.refuse
    }
    fn poll(&mut self) {
        if self.terminal {
            if self.packets.is_empty() {
                self.finish(true);
            }
            return;
        }
        while self.packets.len() < QUEUE_SIZE {
            let Some(session) = &mut self.session else {
                break;
            };
            let event = match session.incoming.try_recv() {
                Ok(event) => event,
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    self.terminal = true;
                    break;
                }
            };
            match event {
                Event::Listening(code) => {
                    self.code = code;
                    self.status = ConnectionStatus::CONNECTED;
                }
                Event::Connected => {
                    self.connected = true;
                    self.status = ConnectionStatus::CONNECTED;
                    let id = self.remote_id();
                    self.base_mut()
                        .emit_signal("peer_connected", &[id.to_variant()]);
                }
                Event::Packet(packet) => self.packets.push_back(packet),
                Event::Disconnected => {
                    self.terminal = true;
                    break;
                }
                Event::Failed(error) => {
                    self.error = error;
                    self.terminal = true;
                    break;
                }
            }
        }
    }
    fn close(&mut self) {
        self.finish(false);
    }
    fn disconnect_peer(&mut self, peer: i32, force: bool) {
        if self.connected && peer == self.remote_id() {
            self.finish(!force);
        }
    }
}

#[godot_api]
impl KyberMultiplayerPeer {
    #[constant]
    const MAX_PACKET_SIZE: i32 = MAX_PACKET as i32;

    #[func]
    fn create_server(&mut self, port: i32, bind_ip: GString) -> Error {
        self.finish(false);
        self.error.clear();
        if !(1..=65535).contains(&port) {
            return self.invalid("Port must be between 1 and 65535");
        }
        let Ok(ip) = bind_ip.to_string().parse::<std::net::IpAddr>() else {
            return self.invalid("Invalid bind IP address");
        };
        self.server = true;
        self.start(Mode::Server((ip, port as u16).into()))
    }
    #[func]
    fn create_client(&mut self, host: GString, port: i32, code: GString) -> Error {
        self.finish(false);
        self.error.clear();
        if !(1..=65535).contains(&port) || host.to_string().trim().is_empty() {
            return self.invalid("Specify a host and a port between 1 and 65535");
        }
        self.server = false;
        self.start(Mode::Client {
            host: host.to_string(),
            port: port as u16,
            code: code.to_string().trim().into(),
        })
    }
    #[func]
    fn get_connection_code(&self) -> GString {
        self.code.as_str().into()
    }
    #[func]
    fn get_last_error(&self) -> GString {
        self.error.as_str().into()
    }

    fn remote_id(&self) -> i32 {
        if self.server { 2 } else { 1 }
    }
    fn invalid(&mut self, error: &str) -> Error {
        self.error = error.into();
        Error::ERR_INVALID_PARAMETER
    }
    fn start(&mut self, mode: Mode) -> Error {
        match Session::start(mode) {
            Ok(session) => {
                session.refuse.store(self.refuse, Ordering::Relaxed);
                self.session = Some(session);
                self.status = ConnectionStatus::CONNECTING;
                Error::OK
            }
            Err(error) => {
                self.error = error.to_string();
                Error::ERR_CANT_CREATE
            }
        }
    }
    fn finish(&mut self, notify: bool) {
        let connected = self.connected;
        let remote = self.remote_id();
        self.session.take();
        self.packets.clear();
        self.code.clear();
        self.connected = false;
        self.terminal = false;
        self.status = ConnectionStatus::DISCONNECTED;
        if connected && notify {
            self.base_mut()
                .emit_signal("peer_disconnected", &[remote.to_variant()]);
        }
    }
}
