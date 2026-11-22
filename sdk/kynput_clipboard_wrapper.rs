// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

use super::{InputTarget, Packet, sys};

#[derive(Debug, PartialEq)]
pub enum TextClipboard {
    Changed,
    Text(String),
    Request,
    Unavailable,
}
impl TextClipboard {
    pub fn to_packet(&self, target: InputTarget) -> Option<Packet> {
        let (kind, text) = match self {
            Self::Changed => (0, ""),
            Self::Text(text) => (1, text.as_str()),
            Self::Request => (2, ""),
            Self::Unavailable => (3, ""),
        };
        let ptr = unsafe {
            sys::kynput_clipboard_packet(target.to_u32(), kind, text.as_ptr(), text.len())
        };
        if ptr.is_null() {
            None
        } else {
            Some(Packet::from_ptr_mut(ptr))
        }
    }
    pub fn from_packet(packet: &Packet) -> Option<Self> {
        let mut data = std::ptr::null();
        let mut len = 0;
        match unsafe { sys::kynput_clipboard_read(packet.to_ptr(), &mut data, &mut len) } {
            0 => Some(Self::Changed),
            1 => Some(Self::Text(
                std::str::from_utf8(unsafe { std::slice::from_raw_parts(data, len) })
                    .ok()?
                    .to_owned(),
            )),
            2 => Some(Self::Request),
            3 => Some(Self::Unavailable),
            _ => None,
        }
    }
}
