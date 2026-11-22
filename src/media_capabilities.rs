// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

use godot::prelude::*;

pub fn get() -> VarDictionary {
    let capabilities = kyber_media::capabilities();
    let mut result = VarDictionary::new();
    result.set("backend", capabilities.backend);
    result.set("h264_decode", capabilities.h264_decode);
    result.set("file_video", capabilities.file_video);
    result.set(
        "h264_encode",
        cfg!(any(
            windows,
            target_os = "macos",
            target_os = "ios",
            target_os = "linux",
            target_os = "android"
        )),
    );
    result.set(
        "h264_acceleration_selection",
        cfg!(any(
            windows,
            target_os = "macos",
            target_os = "ios",
            target_os = "linux",
            target_os = "android"
        )),
    );
    result.set("audio_playback", true);
    result.set(
        "file_audio",
        cfg!(any(
            windows,
            target_os = "macos",
            target_os = "ios",
            target_os = "linux",
            target_os = "android"
        )),
    );
    result.set("jpeg_receive", true);
    result.set("reason", capabilities.reason.as_str());
    result
}
