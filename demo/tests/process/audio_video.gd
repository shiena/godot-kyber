# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends SceneTree

func _initialize() -> void:
    _run.call_deferred()

func _run() -> void:
    var args := OS.get_cmdline_user_args()
    var directory := args[1]
    var sender := args[0] == "sender"
    var address := args[4] if args.size() > 4 else "127.0.0.1"
    var video := KyberFileVideo.new()
    var session := KyberSession.new()
    var player := VideoStreamPlayer.new()
    var audio_player := AudioStreamPlayer.new()
    var audio_stream: KyberAudioStream
    root.add_child(video)
    root.add_child(session)
    root.add_child(player)
    root.add_child(audio_player)
    var capture := AudioEffectCapture.new()
    AudioServer.add_bus()
    var bus := AudioServer.bus_count - 1
    AudioServer.set_bus_name(bus, "KyberAudioTest")
    AudioServer.add_bus_effect(bus, capture)
    player.bus = "KyberAudioTest"
    audio_player.bus = "KyberAudioTest"
    var stream := KyberVideoStream.new()
    if args.size() > 8:
        video.video_codec = 1 if args[8] == "h264" else 0
    if args.size() > 9:
        video.acceleration = int(args[9])
        session.acceleration = int(args[9])
    if sender:
        video.transport_mode = int(args[6]) if args.size() > 6 else 0
        if not video.start_sender(args[2], address, int(args[3]), true):
            quit(1)
            return
        while video.get_connection_code().is_empty():
            if video.has_failed():
                push_error(video.get_status())
                quit(1)
                return
            await process_frame
        var file := FileAccess.open(directory.path_join("code.txt"), FileAccess.WRITE)
        file.store_string(video.get_connection_code())
        file.close()
    else:
        if not session.start_receiver(address, int(args[3]), FileAccess.get_file_as_string(directory.path_join("code.txt"))):
            quit(1)
            return
        if args.size() > 7 and args[7] == "audio-stream":
            stream.audio_enabled = false
            audio_stream = KyberAudioStream.new()
            audio_stream.set_session(session)
            audio_player.stream = audio_stream
            audio_player.play()
        stream.set_session(session)
        player.stream = stream
        player.play()
    var duration := int(args[5]) if args.size() > 5 else (16 if sender else 12)
    var start := Time.get_ticks_msec()
    var captured := 0
    var energy := 0.0
    var peak_sync_ms := 0.0
    var samples: Array = []
    var last_second := -1
    while Time.get_ticks_msec() - start < duration * 1000 and not FileAccess.file_exists(directory.path_join("stop")):
        await process_frame
        var frames := capture.get_frames_available()
        if frames > 0:
            var pcm := capture.get_buffer(frames)
            captured += pcm.size()
            for sample in pcm:
                energy += sample.length_squared()
        var elapsed := Time.get_ticks_msec() - start
        if not sender:
            player.paused = elapsed >= 3000 and elapsed < 3500
            audio_player.stream_paused = player.paused
            if int(elapsed / 1000) != last_second:
                last_second = int(elapsed / 1000)
                var statistics: Dictionary = session.get_statistics()
                samples.append(statistics)
                var sync: Dictionary = statistics.get("performance", {}).get("av_sync", {})
                if elapsed > 5000:
                    peak_sync_ms = maxf(peak_sync_ms, sync.get("p95_ms", 0.0))
    var result: Dictionary = {"video_frames": stream.get_frame_count(), "sender_status": video.get_status(), "receiver_status": session.get_statistics(), "receiver_error": session.get_last_error(), "captured_samples": captured, "energy": energy, "sync_p95_max_ms": peak_sync_ms, "samples": samples}
    var success := sender or (captured > 48000 and energy > 1.0 and stream.get_frame_count() > 50 and peak_sync_ms < 180.0)
    var report := FileAccess.open(directory.path_join(args[0] + ".json"), FileAccess.WRITE)
    report.store_string(JSON.stringify(result))
    report.close()
    if audio_stream != null:
        success = success and audio_stream.get_last_error().is_empty()
    audio_player.stop()
    audio_player.queue_free()
    player.stop()
    session.close()
    video.stop()
    player.queue_free()
    session.queue_free()
    video.queue_free()
    AudioServer.remove_bus(bus)
    await process_frame
    quit(0 if success else 1)
