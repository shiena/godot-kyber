// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// GStreamer codec plugins are supplied by the operating system.
using PacketCallback = void (*)(void*, const uint8_t*, size_t, int64_t, bool);
struct Encoder {
    std::unique_ptr<Pipeline> pipeline;
    std::vector<std::pair<std::string, bool>> candidates;
    size_t candidate = 0;
    uint32_t width, height, fps;
    bool hardware = false, started = false;
    Encoder(uint32_t w, uint32_t h, uint32_t rate, uint32_t bitrate, int mode) : width(w), height(h), fps(rate) {
        initialize();
        auto add = [&](const char* name, std::string options, bool gpu) {
            if (factory(name)) candidates.push_back({std::string(name) + options, gpu});
        };
        auto kbps = std::to_string(bitrate / 1000);
        if (mode != 1) {
            add("vah264enc", " key-int-max=1 b-frames=0 cabac=false dct8x8=false bitrate=" + kbps, true);
            add("nvh264enc", " gop-size=1 bframes=0 zerolatency=true bitrate=" + kbps, true);
            add("vaapih264enc", " keyframe-period=1 max-bframes=0 bitrate=" + kbps, true);
        }
        if (mode != 2) {
            add("x264enc", " tune=zerolatency speed-preset=ultrafast key-int-max=1 bframes=0 byte-stream=true bitrate=" + kbps, false);
            add("openh264enc", " gop-size=1 bitrate=" + std::to_string(bitrate), false);
        }
        if (candidates.empty()) throw std::runtime_error("Install a compatible GStreamer H.264 encoder plugin");
    }
    void open() {
        // OpenH264 accepts I420, while the hardware encoders use NV12.
        const char* format = candidates[candidate].first.rfind("openh264enc", 0) == 0 ? "I420" : "NV12";
        auto desc = "appsrc name=source format=time is-live=true block=false max-buffers=2 caps=\"video/x-raw,format=RGBA,width=" + std::to_string(width)
            + ",height=" + std::to_string(height) + ",framerate=" + std::to_string(fps) + "/1\" ! videoconvert ! video/x-raw,format=" + format + " ! "
            + candidates[candidate].first + " ! h264parse config-interval=-1 ! video/x-h264,stream-format=byte-stream,alignment=au ! appsink name=output sync=false max-buffers=2 drop=false wait-on-eos=false";
        pipeline = std::make_unique<Pipeline>(desc);
        pipeline->start();
        hardware = candidates[candidate].second;
    }
    bool pull(GstClockTime timeout, PacketCallback cb, void* user) {
        pipeline->errors();
        Sample sample{api().gst_app_sink_try_pull_sample(reinterpret_cast<GstAppSink*>(pipeline->sink.value), timeout)};
        if (!sample.value) { pipeline->errors(); return false; }
        auto* buffer = api().gst_sample_get_buffer(sample.value);
        size_t size = api().gst_buffer_get_size(buffer);
        if (!size || size > 16 * 1024 * 1024) throw std::runtime_error("Invalid H.264 encoder output size");
        std::vector<uint8_t> bytes(size);
        if (api().gst_buffer_extract(buffer, 0, bytes.data(), size) != size) throw std::runtime_error("Cannot read H.264 encoder output");
        cb(user, bytes.data(), size, GST_BUFFER_PTS(buffer) / 1000, !GST_BUFFER_FLAG_IS_SET(buffer, GST_BUFFER_FLAG_DELTA_UNIT));
        return true;
    }
    void push(const uint8_t* rgba, size_t len, int64_t pts, PacketCallback cb, void* user) {
        if (len != size_t(width) * height * 4 || pts < 0 || uint64_t(pts) > (UINT64_MAX - 1) / 1000) throw std::runtime_error("Invalid encoder frame");
        for (;;) {
            try {
                if (!pipeline) open();
                auto* buffer = api().gst_buffer_new_allocate(nullptr, len, nullptr);
                if (!buffer) throw std::bad_alloc();
                api().gst_buffer_fill(buffer, 0, rgba, len);
                GST_BUFFER_PTS(buffer) = uint64_t(pts) * 1000;
                GST_BUFFER_DURATION(buffer) = GST_SECOND / fps;
                auto flow = api().gst_app_src_push_buffer(reinterpret_cast<GstAppSrc*>(pipeline->source.value), buffer);
                if (flow != GST_FLOW_OK) { pipeline->errors(); throw std::runtime_error("Encoder input failed"); }
                if (!pull(5 * GST_SECOND, cb, user)) throw std::runtime_error("H.264 encoder output timed out");
                started = true;
                return;
            } catch (...) {
                pipeline.reset();
                if (started || ++candidate >= candidates.size()) throw;
            }
        }
    }
    void drain(PacketCallback cb, void* user) {
        if (!pipeline) return;
        api().gst_app_src_end_of_stream(reinterpret_cast<GstAppSrc*>(pipeline->source.value));
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        while (!pipeline->eos()) {
            pull(10 * GST_MSECOND, cb, user);
            if (std::chrono::steady_clock::now() > deadline) throw std::runtime_error("Encoder drain timed out");
        }
    }
};
extern "C" {
int kg_encoder_create(uint32_t w, uint32_t h, uint32_t fps, uint32_t bitrate, int mode, void** out) {
    return protect([&] { *out = new Encoder(w, h, fps, bitrate, mode); });
}
void kg_encoder_destroy(void* value) { delete static_cast<Encoder*>(value); }
bool kg_encoder_hardware(void* value) { return static_cast<Encoder*>(value)->hardware; }
int kg_encoder_push(void* value, const uint8_t* rgba, size_t len, int64_t pts, PacketCallback cb, void* user) {
    return protect([&] { static_cast<Encoder*>(value)->push(rgba, len, pts, cb, user); });
}
int kg_encoder_drain(void* value, PacketCallback cb, void* user) {
    return protect([&] { static_cast<Encoder*>(value)->drain(cb, user); });
}
}
