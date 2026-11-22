// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

use super::{Packet, sys};

/// Decode upstream rumble packets without changing their wire representation.
pub fn read_rumble(packet: &Packet) -> Option<(u32, u16, u16, i16)> {
    let mut value: sys::kynput_GamepadEventRumble = unsafe { std::mem::zeroed() };
    if unsafe { sys::kynput_gamepad_rumble_from_packet(packet.to_ptr(), &mut value) } {
        Some((
            value.id,
            value.large_motor,
            value.small_motor,
            value.duration,
        ))
    } else {
        None
    }
}
