# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

extends SceneTree

func _initialize() -> void:
    _run.call_deferred()

func _run() -> void:
    var args := OS.get_cmdline_user_args()
    var expect_gpu := args.size() > 0 and args[0] == "gpu"
    var sender := KyberFileVideo.new()
    var receiver := KyberSession.new()
    var player := VideoStreamPlayer.new()
    var source := SubViewport.new()
    var fill := ColorRect.new()
    source.size = Vector2i(640, 360)
    source.render_target_update_mode = SubViewport.UPDATE_ALWAYS
    fill.size = Vector2(640, 360)
    fill.color = Color.RED
    source.add_child(fill)
    root.add_child(source)
    root.add_child(sender)
    root.add_child(receiver)
    root.add_child(player)
    player.expand = true
    player.size = Vector2(640, 360)
    sender.video_codec = 1
    sender.acceleration = 2
    receiver.acceleration = 2
    if not sender.start_viewport_sender(source, "127.0.0.1", 14710, 24):
        push_error(sender.get_status())
        quit(1)
        return
    var deadline := Time.get_ticks_msec() + 40000
    while sender.get_connection_code().is_empty():
        if Time.get_ticks_msec() > deadline or sender.has_failed():
            push_error(sender.get_status())
            quit(1)
            return
        await process_frame
    for cycle in range(3):
        var expected := Color.RED if cycle % 2 == 0 else Color.BLUE
        fill.color = expected
        receiver.start_receiver("127.0.0.1", 14710, sender.get_connection_code())
        var stream := KyberVideoStream.new()
        stream.set_session(receiver)
        player.stream = stream
        player.play()
        while stream.get_frame_count() < 30:
            if Time.get_ticks_msec() > deadline or not receiver.get_last_error().is_empty():
                push_error("GPU video timeout: " + receiver.get_status() + " " + str(receiver.get_statistics()))
                quit(1)
                return
            await process_frame
        var texture := receiver.get_texture()
        var stats := receiver.get_statistics()
        var shared := texture is Texture2DRD
        if shared != expect_gpu:
            push_error("Unexpected texture path: " + str(stats))
            quit(1)
            return
        var image := receiver.latest_image()
        if image == null or image.get_width() != 640 or image.get_height() != 360:
            push_error("Missing GPU readback image")
            quit(1)
            return
        var color := image.get_pixel(320, 180)
        if not _matches(color, expected):
            push_error("Unexpected decoded color: " + str(color))
            quit(1)
            return
        var playback_image := player.get_video_texture().get_image()
        if playback_image == null or not _matches(playback_image.get_pixel(320, 180), expected):
            push_error("Missing playback texture image")
            quit(1)
            return
        await RenderingServer.frame_post_draw
        var displayed := root.get_texture().get_image().get_pixel(10, 10)
        if not _matches(displayed, expected):
            push_error("Unexpected displayed color: " + str(displayed))
            quit(1)
            return
        print("GPU_VIDEO_CYCLE_OK ", cycle, " ", stats.performance.get("gpu_transfer", "cpu"))
        player.stop()
        player.stream = null
        receiver.close()
        for frame in range(8):
            await process_frame
    sender.stop()
    sender.queue_free()
    receiver.queue_free()
    player.queue_free()
    source.queue_free()
    for frame in range(8):
        await process_frame
    print("GPU_VIDEO_OK")
    quit(0)

func _matches(actual: Color, expected: Color) -> bool:
    return absf(actual.r - expected.r) < 0.2 and absf(actual.g - expected.g) < 0.2 and absf(actual.b - expected.b) < 0.2
