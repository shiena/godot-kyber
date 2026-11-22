# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends SceneTree

func _initialize() -> void:
    _run.call_deferred()

func _run() -> void:
    var args := OS.get_cmdline_user_args()
    var directory := args[0]
    var sender := KyberFileVideo.new()
    var session := KyberSession.new()
    var receiver := AudioStreamPlayer.new()
    root.add_child(sender)
    root.add_child(session)
    root.add_child(receiver)
    AudioServer.add_bus()
    var source_bus := AudioServer.bus_count - 1
    AudioServer.set_bus_name(source_bus, "KyberSourceTest")
    AudioServer.set_bus_mute(source_bus, true)
    var generator := AudioStreamGenerator.new()
    generator.mix_rate = 44100
    generator.buffer_length = 0.1
    var tone := AudioStreamPlayer.new()
    tone.stream = generator
    tone.bus = "KyberSourceTest"
    root.add_child(tone)
    tone.play()
    var playback: AudioStreamGeneratorPlayback = tone.get_stream_playback()
    AudioServer.add_bus()
    var target_bus := AudioServer.bus_count - 1
    AudioServer.set_bus_name(target_bus, "KyberReceiveTest")
    var capture := AudioEffectCapture.new()
    AudioServer.add_bus_effect(target_bus, capture)
    receiver.bus = "KyberReceiveTest"
    sender.transport_mode = int(args[1]) if args.size() > 1 else 0
    assert(sender.start_audio_sender("127.0.0.1", 14480))
    assert(sender.capture_audio_bus("KyberSourceTest"))
    while sender.get_connection_code().is_empty():
        await process_frame
    var standalone := args.size() > 3 and args[3] == "standalone"
    if not standalone:
        assert(session.start_receiver("127.0.0.1", int(args[2]) if args.size() > 2 else 14480, sender.get_connection_code()))
    var stream := KyberAudioStream.new()
    if standalone:
        stream.host = "127.0.0.1"
        stream.port = int(args[2]) if args.size() > 2 else 14480
        stream.set_connection_code(sender.get_connection_code())
    else:
        stream.set_session(session)
    receiver.stream = stream
    receiver.play()
    var duplicate: AudioStreamPlayer
    var duplicate_stream: KyberAudioStream
    var duplicate_checked := standalone
    var restarted := false
    var start := Time.get_ticks_msec()
    var index := 0
    var samples := 0
    var energy := 0.0
    while Time.get_ticks_msec() - start < (20000 if standalone else 8000):
        if not standalone and duplicate == null and not duplicate_checked and Time.get_ticks_msec() - start > 1000:
            duplicate_stream = KyberAudioStream.new()
            duplicate_stream.set_session(session)
            duplicate = AudioStreamPlayer.new()
            duplicate.stream = duplicate_stream
            root.add_child(duplicate)
            duplicate.play()
        if duplicate != null and not duplicate_checked and Time.get_ticks_msec() - start > 2000:
            assert(duplicate_stream.get_last_error().contains("consumer"))
            duplicate.stop()
            duplicate.queue_free()
            duplicate_stream = null
            duplicate_checked = true
        if not restarted and Time.get_ticks_msec() - start > 3000:
            receiver.stop()
            receiver.play()
            restarted = true
        var available := playback.get_frames_available()
        var buffer := PackedVector2Array()
        buffer.resize(available)
        for i in range(available):
            var value := sin(index * TAU * 880.0 / 44100.0) * 0.2
            buffer[i] = Vector2(value, value)
            index += 1
        playback.push_buffer(buffer)
        var pcm := capture.get_buffer(capture.get_frames_available())
        samples += pcm.size()
        for value in pcm:
            energy += value.length_squared()
        await process_frame
    assert(standalone or session.is_active())
    assert(stream.get_last_error().is_empty(), stream.get_last_error())
    receiver.stop()
    session.close()
    sender.stop()
    tone.stop()
    capture = null
    playback = null
    stream = null
    assert(AudioServer.get_bus_effect_count(source_bus) == 0)
    var result := {"samples": samples, "energy": energy, "success": samples > 100000 and energy > 10.0}
    FileAccess.open(directory.path_join("live-audio.json"), FileAccess.WRITE).store_string(JSON.stringify(result))
    for node in [sender, session, receiver, tone]:
        node.queue_free()
    AudioServer.remove_bus(target_bus)
    AudioServer.remove_bus(source_bus)
    await process_frame
    quit(0 if result.success else 1)
