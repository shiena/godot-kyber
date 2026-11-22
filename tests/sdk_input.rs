// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

use kynput_rs::{GamepadButton, GamepadEvent, InputTarget, TextClipboard};

#[test]
fn native_clipboard_packets_round_trip_unicode_and_limits() {
    for event in [
        TextClipboard::Changed,
        TextClipboard::Request,
        TextClipboard::Unavailable,
        TextClipboard::Text("日本語\nclipboard 🦀".into()),
        TextClipboard::Text(String::new()),
        TextClipboard::Text("x".repeat(60 * 1024)),
    ] {
        let packet = event.to_packet(InputTarget::Host).unwrap();
        assert_eq!(TextClipboard::from_packet(&packet), Some(event));
    }
    assert!(
        TextClipboard::Text("x".repeat(60 * 1024 + 1))
            .to_packet(InputTarget::Host)
            .is_none()
    );
}

#[test]
fn right_stick_click_is_not_a_shoulder_button() {
    let packet = GamepadEvent::Button {
        id: 3,
        type_: GamepadButton::RightStick,
        pressed: true,
    }
    .to_packet(InputTarget::Host)
    .unwrap();
    match GamepadEvent::from_packet(&packet).unwrap() {
        GamepadEvent::Button { id, type_, pressed } => {
            assert_eq!(id, 3);
            assert!(matches!(type_, GamepadButton::RightStick));
            assert!(pressed);
        }
        _ => panic!("Expected a button event"),
    }
}

#[test]
fn upstream_rumble_packets_preserve_motors_and_duration() {
    for (strong, weak, duration) in [(65535, 1234, 150), (0, 0, 0), (42, 99, -1)] {
        let native = Box::new(kynput::InputPacket::new(
            kynput::InputTarget::Client,
            kynput::Payload::GamepadEvent(kynput::GamepadEvent::Rumble {
                id: 3,
                large_motor: strong,
                small_motor: weak,
                duration,
            }),
        ));
        let packet = kynput_rs::Packet::from_ptr_mut(Box::into_raw(native).cast());
        assert_eq!(
            kynput_rs::read_rumble(&packet),
            Some((3, strong, weak, duration))
        );
    }
    let packet = TextClipboard::Request.to_packet(InputTarget::Host).unwrap();
    assert_eq!(kynput_rs::read_rumble(&packet), None);
}
