// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// Text-only access to the SDK's native clipboard packets.
use super::{InputPacket, InputTarget};
use crate::types::{ClipboardData, ClipboardFormat, MAX_CLIPBOARD_SIZE};
use crate::{ClipboardEvent, Payload};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kynput_clipboard_packet(
    target: InputTarget,
    kind: u32,
    data: *const u8,
    len: usize,
) -> Option<Box<InputPacket>> {
    let event = match kind {
        0 => ClipboardEvent::Change {
            formats: vec![ClipboardFormat::Text],
        },
        1 => {
            if len > MAX_CLIPBOARD_SIZE || data.is_null() {
                return None;
            }
            let text =
                std::str::from_utf8(unsafe { std::slice::from_raw_parts(data, len) }).ok()?;
            ClipboardEvent::Data(ClipboardData::Text(text.to_owned()))
        }
        2 => ClipboardEvent::DataRequest {
            format: ClipboardFormat::Text,
        },
        3 => ClipboardEvent::DataUnavailable {
            format: ClipboardFormat::Text,
        },
        _ => return None,
    };
    Some(Box::new(InputPacket::new(
        target,
        Payload::Clipboard(event),
    )))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kynput_clipboard_read(
    packet: &InputPacket,
    data: *mut *const u8,
    len: *mut usize,
) -> i32 {
    match &packet.payload {
        Payload::Clipboard(ClipboardEvent::Change { formats }) => {
            if formats.contains(&ClipboardFormat::Text) {
                0
            } else {
                3
            }
        }
        Payload::Clipboard(ClipboardEvent::Data(ClipboardData::Text(text))) => {
            if text.len() > MAX_CLIPBOARD_SIZE || data.is_null() || len.is_null() {
                return -1;
            }
            unsafe {
                *data = text.as_ptr();
                *len = text.len();
            }
            1
        }
        Payload::Clipboard(ClipboardEvent::DataRequest {
            format: ClipboardFormat::Text,
        }) => 2,
        Payload::Clipboard(ClipboardEvent::DataUnavailable { .. }) => 3,
        _ => -1,
    }
}
