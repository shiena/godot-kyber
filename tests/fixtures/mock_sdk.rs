// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// Linked SDK callback fixture. This does not implement networking or decoding.
use std::ffi::{CStr, c_char, c_void};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;

type Event = unsafe extern "C" fn(*mut c_void, *const u8, usize);
type Audio = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
type Frame = unsafe extern "C" fn(*mut c_void, *const u8, usize, u32, u32, u64);
struct Session {
    event: Event,
    frame: Frame,
    audio: Audio,
    userdata: usize,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    reconnect: bool,
    failure: bool,
    runs: u32,
    clipboard_enabled: bool,
    clipboard_text: String,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kyclient_godot_create(
    config: *const c_char,
    event: Event,
    frame: Frame,
    audio: Audio,
    userdata: *mut c_void,
) -> *mut c_void {
    let config = unsafe { CStr::from_ptr(config) }.to_string_lossy();
    let reconnect =
        config.contains("sdk-reconnect") && config.contains("\"auto_reconnection\":true");
    let failure = config.contains("sdk-reconnect-fail");
    let data = br#"{"kind":"connected","displays":[{"id":42,"width":640,"height":360}]}"#;
    unsafe {
        event(userdata, data.as_ptr(), data.len());
    }
    Box::into_raw(Box::new(Session {
        event,
        frame,
        audio,
        userdata: userdata as usize,
        stop: Arc::new(false.into()),
        worker: None,
        reconnect,
        failure,
        runs: 0,
        clipboard_enabled: config.contains("\"clipboard_enabled\":true"),
        clipboard_text: String::new(),
    }))
    .cast()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kyclient_godot_start(
    handle: *mut c_void,
    display: u32,
    width: u32,
    height: u32,
    _: u32,
    enable_audio: bool,
) -> i32 {
    let state = unsafe { &mut *handle.cast::<Session>() };
    if display != 42 || (state.worker.is_some() && !(state.reconnect && state.runs == 1)) {
        return -1;
    }
    if let Some(worker) = state.worker.take() {
        worker.join().unwrap();
    }
    state.runs += 1;
    let reconnect = state.reconnect && state.runs == 1;
    let failure = state.failure;
    let (event, frame, audio, userdata, stop) = (
        state.event,
        state.frame,
        state.audio,
        state.userdata,
        state.stop.clone(),
    );
    state.worker = Some(std::thread::spawn(move || {
        let started = br#"{"kind":"streaming"}"#;
        unsafe {
            event(userdata as *mut c_void, started.as_ptr(), started.len());
        }
        let pixels = [25, 100, 200, 255].repeat((width * height) as usize);
        let begin = std::time::Instant::now();
        let mut encoder =
            opus::Encoder::new(48000, opus::Channels::Stereo, opus::Application::Audio).unwrap();
        let mut sample_index = 0u64;
        let mut next_audio = std::time::Duration::ZERO;
        if enable_audio {
            let codec = kymux_types::CodecPacketHeader {
                codec: u32::from_be_bytes(*b"opus"),
                rotation: 0,
                frame_size: 0,
            }
            .serialize();
            unsafe {
                audio(userdata as _, codec.as_ptr(), codec.len());
            }
        }
        while !stop.load(Ordering::Acquire) {
            if reconnect && begin.elapsed() > std::time::Duration::from_millis(100) {
                // Fixture events mirror the SDK; this is not a network reconnection test.
                let events: &[&[u8]] = if failure {
                    &[
                        br#"{"kind":"transport_interrupted"}"#,
                        br#"{"kind":"reconnecting","attempt":10,"max_attempts":10}"#,
                        br#"{"kind":"error","message":"Kyber: ReconnectionFailed"}"#,
                        br#"{"kind":"stopped"}"#,
                    ]
                } else {
                    &[
                        br#"{"kind":"transport_interrupted"}"#,
                        br#"{"kind":"reconnecting","attempt":1,"max_attempts":10}"#,
                        br#"{"kind":"reconnected","should_restart_streaming":true}"#,
                    ]
                };
                for message in events {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    unsafe {
                        event(userdata as *mut c_void, message.as_ptr(), message.len());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                break;
            }
            if enable_audio && begin.elapsed() >= next_audio {
                let pcm: Vec<f32> = (0..960)
                    .flat_map(|i| {
                        let value = ((sample_index + i) as f32 * std::f32::consts::TAU * 880.0
                            / 48000.0)
                            .sin()
                            * 0.2;
                        [value, value]
                    })
                    .collect();
                let mut payload = vec![0u8; 4000];
                let size = encoder.encode_float(&pcm, &mut payload).unwrap();
                payload.truncate(size);
                let mut bytes = kymux_types::MediaPacketHeader {
                    is_config: false,
                    is_key: true,
                    pts: sample_index * 1_000_000 / 48000,
                    size: size as u32,
                }
                .serialize()
                .to_vec();
                bytes.extend(payload);
                unsafe {
                    audio(userdata as _, bytes.as_ptr(), bytes.len());
                }
                sample_index += 960;
                next_audio += std::time::Duration::from_millis(20);
            }
            unsafe {
                frame(
                    userdata as *mut c_void,
                    pixels.as_ptr(),
                    pixels.len(),
                    width,
                    height,
                    begin.elapsed().as_micros() as u64,
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kyclient_godot_input(
    handle: *mut c_void,
    kind: u32,
    _: i32,
    _: i32,
    pressed: bool,
) -> i32 {
    let session = unsafe { &*handle.cast::<Session>() };
    if session.worker.is_some() {
        if kind == 4 && pressed {
            let data =
                br#"{"kind":"gamepad_rumble","device":0,"weak":65535,"strong":0,"duration_ms":-1}"#;
            unsafe {
                (session.event)(session.userdata as *mut c_void, data.as_ptr(), data.len());
            }
        }
        0
    } else {
        -1
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kyclient_godot_destroy(handle: *mut c_void) {
    let mut state = unsafe { Box::from_raw(handle.cast::<Session>()) };
    state.stop.store(true, Ordering::Release);
    if let Some(worker) = state.worker.take() {
        worker.join().unwrap();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kyclient_godot_clipboard(
    handle: *mut c_void,
    operation: u32,
    text: *const u8,
    len: usize,
) -> i32 {
    let state = unsafe { &mut *handle.cast::<Session>() };
    if !state.clipboard_enabled || len > 60 * 1024 {
        return -1;
    }
    let event = match operation {
        0 => {
            if text.is_null() {
                return -1;
            }
            let Ok(text) = std::str::from_utf8(unsafe { std::slice::from_raw_parts(text, len) })
            else {
                return -1;
            };
            state.clipboard_text = text.to_owned();
            serde_json::json!({"kind":"clipboard_available"})
        }
        1 => serde_json::json!({"kind":"clipboard_text", "message":state.clipboard_text}),
        _ => return -1,
    }
    .to_string();
    unsafe {
        (state.event)(state.userdata as *mut c_void, event.as_ptr(), event.len());
    }
    0
}
