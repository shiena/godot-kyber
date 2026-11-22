# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends GutTest

func test_audio_resource_does_not_save_credentials() -> void:
    var stream := KyberAudioStream.new()
    stream.host = "127.0.0.1"
    stream.set_connection_code("private-code")
    assert_eq(ResourceSaver.save(stream, "user://kyber-audio-test.tres"), OK)
    var text := FileAccess.get_file_as_string("user://kyber-audio-test.tres")
    assert_false(text.contains("private-code"))
    assert_eq(stream.get_length(), 0.0)
    DirAccess.remove_absolute("user://kyber-audio-test.tres")

func test_capture_requires_live_sender_and_valid_bus() -> void:
    var sender := KyberFileVideo.new()
    add_child_autofree(sender)
    assert_false(sender.capture_audio_bus("Master"))
    assert_false(sender.capture_microphone())
    sender.transport_mode = 99
    assert_false(sender.start_audio_sender("127.0.0.1", 14482))
    sender.transport_mode = 0
    assert_true(sender.start_audio_sender("127.0.0.1", 14482))
    assert_false(sender.capture_audio_bus("missing-kyber-bus"))
    sender.stop()

func test_bus_capture_cleanup_follows_bus_reordering() -> void:
    var sender := KyberFileVideo.new()
    add_child_autofree(sender)
    AudioServer.add_bus()
    var index := AudioServer.bus_count - 1
    AudioServer.set_bus_name(index, "KyberCleanupTest")
    assert_true(sender.start_audio_sender("127.0.0.1", 14483))
    assert_true(sender.capture_audio_bus("KyberCleanupTest"))
    assert_eq(AudioServer.get_bus_effect_count(index), 1)
    AudioServer.add_bus(1)
    sender.stop_audio_capture()
    index = AudioServer.get_bus_index("KyberCleanupTest")
    assert_eq(AudioServer.get_bus_effect_count(index), 0)
    sender.stop()
    AudioServer.remove_bus(index)
    AudioServer.remove_bus(1)

func test_microphone_requires_input_setting() -> void:
    var previous = ProjectSettings.get_setting("audio/driver/enable_input")
    ProjectSettings.set_setting("audio/driver/enable_input", false)
    var sender := KyberFileVideo.new()
    add_child_autofree(sender)
    assert_true(sender.start_audio_sender("127.0.0.1", 14484))
    assert_false(sender.capture_microphone())
    assert_true(sender.get_status().contains("enable_input"))
    sender.stop()
    ProjectSettings.set_setting("audio/driver/enable_input", previous)

func test_standalone_audio_reports_invalid_connection() -> void:
    var stream := KyberAudioStream.new()
    stream.set_connection_code("invalid-code")
    var playback := stream.instantiate_playback()
    playback.start()
    assert_false(playback.is_playing())
    assert_false(stream.get_last_error().is_empty())
    playback.stop()
