// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

use godot::{
    classes::{
        InputEvent, InputEventJoypadButton, InputEventJoypadMotion, InputEventKey,
        InputEventMouseButton, InputEventMouseMotion,
    },
    global::Key,
    prelude::*,
};

pub fn translate(event: Gd<InputEvent>) -> Option<(u32, i32, i32, bool)> {
    if let Ok(key) = event.clone().try_cast::<InputEventKey>() {
        if key.is_echo() {
            return None;
        }
        let code = if key.get_physical_keycode() != Key::NONE {
            key.get_physical_keycode()
        } else {
            key.get_keycode()
        };
        return hid(code).map(|code| (0, code, 0, key.is_pressed()));
    }
    if let Ok(motion) = event.clone().try_cast::<InputEventMouseMotion>() {
        let delta = motion.get_relative();
        if !delta.is_finite() {
            return None;
        }
        return Some((
            1,
            delta.x.round().clamp(-32768.0, 32767.0) as i32,
            delta.y.round().clamp(-32768.0, 32767.0) as i32,
            false,
        ));
    }
    if let Ok(button) = event.clone().try_cast::<InputEventJoypadButton>() {
        return gamepad_button(
            button.get_device(),
            button.get_button_index().ord(),
            button.is_pressed(),
        );
    }
    if let Ok(axis) = event.clone().try_cast::<InputEventJoypadMotion>() {
        return gamepad_axis(
            axis.get_device(),
            axis.get_axis().ord(),
            axis.get_axis_value(),
        );
    }
    if let Ok(button) = event.try_cast::<InputEventMouseButton>() {
        let index = button.get_button_index().ord();
        if (1..=3).contains(&index) {
            return Some((2, index - 1, 0, button.is_pressed()));
        }
        if button.is_pressed() && (4..=7).contains(&index) {
            let factor = button.get_factor();
            if !factor.is_finite() {
                return None;
            }
            let factor = factor.clamp(0.0, 1.0);
            let (dx, dy) = match index {
                4 => (0.0, -factor),
                5 => (0.0, factor),
                6 => (-factor, 0.0),
                _ => (factor, 0.0),
            };
            return wheel(dx, dy);
        }
    }
    None
}

pub fn wheel(dx: f32, dy: f32) -> Option<(u32, i32, i32, bool)> {
    if !dx.is_finite() || !dy.is_finite() || dx.abs() > 1.0 || dy.abs() > 1.0 {
        return None;
    }
    Some((
        crate::backend::INPUT_WHEEL,
        dx.to_bits() as i32,
        dy.to_bits() as i32,
        false,
    ))
}

pub fn gamepad_button(device: i32, button: i32, pressed: bool) -> Option<(u32, i32, i32, bool)> {
    if !(0..32).contains(&device) || !(0..15).contains(&button) {
        return None;
    }
    Some((4, (device << 16) | button, 0, pressed))
}

pub fn gamepad_axis(device: i32, axis: i32, value: f32) -> Option<(u32, i32, i32, bool)> {
    if !(0..32).contains(&device)
        || !(0..6).contains(&axis)
        || !value.is_finite()
        || !(-1.0..=1.0).contains(&value)
        || (axis >= 4 && value < 0.0)
    {
        return None;
    }
    // Both APIs use nonnegative trigger values.
    let value = if axis >= 4 { value.max(0.0) } else { value };
    let value = if value < 0.0 {
        value * 32768.0
    } else {
        value * 32767.0
    };
    Some((5, (device << 16) | axis, value.round() as i32, false))
}

fn hid(key: Key) -> Option<i32> {
    let code = key.ord();
    if (Key::A.ord()..=Key::Z.ord()).contains(&code) {
        return Some(4 + code - Key::A.ord());
    }
    if (Key::KEY_1.ord()..=Key::KEY_9.ord()).contains(&code) {
        return Some(30 + code - Key::KEY_1.ord());
    }
    if (Key::F1.ord()..=Key::F12.ord()).contains(&code) {
        return Some(58 + code - Key::F1.ord());
    }
    Some(match key {
        Key::KEY_0 => 39,
        Key::ENTER => 40,
        Key::ESCAPE => 41,
        Key::BACKSPACE => 42,
        Key::TAB => 43,
        Key::SPACE => 44,
        Key::MINUS => 45,
        Key::EQUAL => 46,
        Key::BRACKETLEFT => 47,
        Key::BRACKETRIGHT => 48,
        Key::BACKSLASH => 49,
        Key::SEMICOLON => 51,
        Key::APOSTROPHE => 52,
        Key::QUOTELEFT => 53,
        Key::COMMA => 54,
        Key::PERIOD => 55,
        Key::SLASH => 56,
        Key::CAPSLOCK => 57,
        Key::INSERT => 73,
        Key::HOME => 74,
        Key::PAGEUP => 75,
        Key::DELETE => 76,
        Key::END => 77,
        Key::PAGEDOWN => 78,
        Key::RIGHT => 79,
        Key::LEFT => 80,
        Key::DOWN => 81,
        Key::UP => 82,
        Key::CTRL => 224,
        Key::SHIFT => 225,
        Key::ALT => 226,
        Key::META => 227,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_gamepad_ranges_and_trigger_scaling() {
        assert_eq!(gamepad_axis(2, 0, -1.0), Some((5, 2 << 16, -32768, false)));
        assert_eq!(gamepad_axis(0, 4, 0.0).unwrap().2, 0);
        assert_eq!(gamepad_axis(0, 4, 1.0).unwrap().2, 32767);
        assert!(gamepad_axis(0, 0, f32::NAN).is_none());
        assert!(gamepad_axis(32, 0, 0.0).is_none());
        assert!(gamepad_button(0, 15, true).is_none());
        assert!(gamepad_button(-1, 0, true).is_none());
    }

    #[test]
    fn wheel_preserves_fractional_steps_and_rejects_invalid_values() {
        let (_, dx, dy, _) = wheel(0.25, -1.0).unwrap();
        assert_eq!(f32::from_bits(dx as u32), 0.25);
        assert_eq!(f32::from_bits(dy as u32), -1.0);
        assert!(wheel(f32::NAN, 0.0).is_none());
        assert!(wheel(0.0, f32::INFINITY).is_none());
        assert!(wheel(2.0, 0.0).is_none());
    }

    #[test]
    fn translates_keyboard_usages_without_treating_godot_codes_as_hid() {
        for (key, usage) in [
            (Key::A, 4),
            (Key::Z, 29),
            (Key::KEY_1, 30),
            (Key::KEY_0, 39),
            (Key::F12, 69),
            (Key::LEFT, 80),
            (Key::CTRL, 224),
        ] {
            assert_eq!(hid(key), Some(usage));
        }
        assert_eq!(hid(Key::NONE), None);
    }
}
