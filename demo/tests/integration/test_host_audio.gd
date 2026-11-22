# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends GutTest

func _receive_host_audio(separate_audio: bool, reconnect: bool = false) -> void:
    var session := KyberSession.new()
    add_child_autofree(session)
    session.auto_reconnect = reconnect
    AudioServer.add_bus()
    var bus := AudioServer.bus_count - 1
    AudioServer.set_bus_name(bus, "KyberHostAudioTest")
    var capture := AudioEffectCapture.new()
    AudioServer.add_bus_effect(bus, capture)
    var video := VideoStreamPlayer.new()
    add_child_autofree(video)
    video.bus = "KyberHostAudioTest"
    var audio := AudioStreamPlayer.new()
    add_child_autofree(audio)
    audio.bus = "KyberHostAudioTest"
    assert_true(session.connect_to_host("sdk-reconnect" if reconnect else "sdk-audio", 8080, "", "", ""))
    assert_true(await wait_until(func(): return session.get_state() == KyberSession.CONNECTED, 2.0))
    assert_true(session.start_stream(42, 64, 32, 1000000, true))
    var stream := KyberVideoStream.new()
    stream.audio_enabled = not separate_audio
    stream.set_session(session)
    video.stream = stream
    var audio_stream := KyberAudioStream.new()
    if separate_audio:
        audio_stream.set_session(session)
        audio.stream = audio_stream
        audio.play()
    video.play()
    var energy := 0.0
    var deadline := Time.get_ticks_msec() + 1800
    while Time.get_ticks_msec() < deadline:
        await wait_process_frames(1)
        var pcm := capture.get_buffer(capture.get_frames_available())
        for sample in pcm:
            energy += sample.length_squared()
    assert_gt(energy, 1.0, "SDK Opus packets must reach the Godot audio bus")
    assert_eq(session.get_state(), KyberSession.STREAMING)
    assert_gt(stream.get_frame_count(), 2)
    assert_true(audio_stream.get_last_error().is_empty())
    assert_true(stream.get_last_error().is_empty())
    var performance: Dictionary = session.get_statistics().performance
    assert_gt(performance.audio_received_samples, 48000)
    assert_gt(performance.audio_playback_pts_us, 0)
    audio.stop()
    video.stop()
    session.close()
    AudioServer.remove_bus(bus)

func test_sdk_audio_plays_through_video_player() -> void:
    await _receive_host_audio(false)

func test_sdk_audio_shares_session_with_audio_stream() -> void:
    await _receive_host_audio(true)

func test_sdk_reconnect_restores_requested_audio() -> void:
    await _receive_host_audio(true, true)
