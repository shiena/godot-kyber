// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// Shared by the embedded SDK adapter and its real-QUIC integration test.
async fn forward_audio(
    mut protocol: types::AudioClientProtocol,
    mut emit: impl FnMut(&[u8]) -> bool,
) -> std::result::Result<(), String> {
    while let Some(packet) = protocol.recv().await.map_err(|e| e.to_string())? {
        let bytes = match packet {
            types::AVPacket::Codec(p) => p.header.serialize().to_vec(),
            types::AVPacket::Media(p) => {
                if p.payload.len() > 4000 {
                    return Err("Host audio packet exceeds limit".into());
                }
                let mut data = p.header.serialize().to_vec();
                data.extend_from_slice(&p.payload);
                data
            }
            types::AVPacket::Hole(p) => p.header.serialize().to_vec(),
        };
        if !emit(&bytes) {
            return Ok(());
        }
    }
    Ok(())
}
