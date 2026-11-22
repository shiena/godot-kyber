// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// GStreamer is supplied by the system. Resolve it lazily so RPC and JPEG
// reception remain usable when the optional media runtime is absent.
#include <gst/gst.h>
#include <gst/app/gstappsrc.h>
#include <gst/app/gstappsink.h>
#include <gst/video/video.h>
#include <gst/pbutils/pbutils.h>
#include <dlfcn.h>
#include <chrono>
#include <cstring>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <string>
#include <vector>

using Callback = void (*)(void*, const uint8_t*, size_t, uint32_t, uint32_t, int64_t);
static thread_local std::string last_error;
struct Library {
    void* handle;
    bool keep = false;
    explicit Library(const char* name) : handle(dlopen(name, RTLD_NOW | RTLD_LOCAL)) {
        if (!handle) throw std::runtime_error(std::string("GStreamer unavailable: ") + dlerror());
    }
    ~Library() { if (!keep) dlclose(handle); }
    template<class T> T get(const char* name) {
        void* address = dlsym(handle, name);
        if (!address) throw std::runtime_error(std::string("Missing GStreamer symbol: ") + name);
        return reinterpret_cast<T>(address);
    }
};
#define LOAD(library, name) decltype(&::name) name = library.get<decltype(&::name)>(#name)
struct Api {
    Library core{"libgstreamer-1.0.so.0"}, app{"libgstapp-1.0.so.0"}, video{"libgstvideo-1.0.so.0"};
    LOAD(core, gst_init_check);
    LOAD(core, gst_parse_launch);
    LOAD(core, gst_element_factory_find);
    LOAD(core, gst_object_unref);
    LOAD(core, gst_mini_object_unref);
    LOAD(core, gst_element_get_bus);
    LOAD(core, gst_bus_pop_filtered);
    LOAD(core, gst_message_parse_error);
    LOAD(core, gst_element_set_state);
    LOAD(core, gst_element_get_state);
    LOAD(core, gst_bin_get_by_name);
    LOAD(core, gst_buffer_new_allocate);
    LOAD(core, gst_buffer_fill);
    LOAD(core, gst_sample_get_caps);
    LOAD(core, gst_sample_get_buffer);
    LOAD(core, g_error_free);
    LOAD(core, g_free);
    LOAD(core, g_filename_to_uri);
    LOAD(core, g_object_set);
    LOAD(core, g_object_unref);
    LOAD(core, gst_buffer_get_size);
    LOAD(core, gst_buffer_extract);
    LOAD(app, gst_app_src_push_buffer);
    LOAD(app, gst_app_src_end_of_stream);
    LOAD(app, gst_app_src_get_current_level_buffers);
    LOAD(app, gst_app_sink_try_pull_sample);
    LOAD(app, gst_app_sink_is_eos);
    LOAD(video, gst_video_info_from_caps);
    LOAD(video, gst_video_frame_map);
    LOAD(video, gst_video_frame_unmap);
    Api() {
        // GStreamer has process-wide plugin threads and type registrations.
        core.keep = app.keep = video.keep = true;
    }
};
#undef LOAD
static Api& api() { static Api instance; return instance; }
template<class F> static int protect(F f) noexcept {
    try { f(); last_error.clear(); return 0; }
    catch (const std::exception& e) { last_error = e.what(); return -1; }
    catch (...) { last_error = "Unknown GStreamer failure"; return -1; }
}
static std::string take_error(GError* error, const char* fallback) {
    if (!error) return fallback;
    std::string message = error->message;
    api().g_error_free(error);
    return message;
}
static void initialize() {
    static std::once_flag initialized;
    std::call_once(initialized, [] {
        GError* error = nullptr;
        if (!api().gst_init_check(nullptr, nullptr, &error))
            throw std::runtime_error(take_error(error, "Cannot initialize GStreamer"));
    });
}
static bool factory(const char* name) {
    auto* value = api().gst_element_factory_find(name);
    if (value) api().gst_object_unref(value);
    return value != nullptr;
}
static const char* decoder_name(int mode = 1) {
    if (mode != 1) {
        for (const char* name : {"vah264dec", "nvh264dec", "vaapih264dec"}) if (factory(name)) return name;
        if (mode == 2) throw std::runtime_error("No GStreamer hardware H.264 decoder is installed");
    }
    if (factory("avdec_h264")) return "avdec_h264";
    if (factory("openh264dec")) return "openh264dec";
    throw std::runtime_error("Install a GStreamer H.264 decoder (avdec_h264 or openh264dec)");
}
struct Object {
    GstElement* value = nullptr;
    ~Object() { if (value) api().gst_object_unref(value); }
};
struct Sample {
    GstSample* value;
    ~Sample() { if (value) api().gst_mini_object_unref(reinterpret_cast<GstMiniObject*>(value)); }
};
struct Pipeline {
    Object pipeline, source, sink;
    explicit Pipeline(const std::string& description) {
        initialize();
        GError* error = nullptr;
        pipeline.value = api().gst_parse_launch(description.c_str(), &error);
        if (error || !pipeline.value) throw std::runtime_error(take_error(error, "Cannot create GStreamer pipeline"));
        source.value = api().gst_bin_get_by_name(reinterpret_cast<GstBin*>(pipeline.value), "source");
        sink.value = api().gst_bin_get_by_name(reinterpret_cast<GstBin*>(pipeline.value), "output");
        if (!source.value || !sink.value) throw std::runtime_error("Missing pipeline endpoint");
    }
    ~Pipeline() { api().gst_element_set_state(pipeline.value, GST_STATE_NULL); }
    void start() {
        if (api().gst_element_set_state(pipeline.value, GST_STATE_PLAYING) == GST_STATE_CHANGE_FAILURE) {
            errors();
            throw std::runtime_error("Cannot start GStreamer pipeline");
        }
    }
    void errors() {
        auto* bus = api().gst_element_get_bus(pipeline.value);
        auto* message = api().gst_bus_pop_filtered(bus, GST_MESSAGE_ERROR);
        api().gst_object_unref(bus);
        if (message) {
            GError* error = nullptr;
            gchar* debug = nullptr;
            api().gst_message_parse_error(message, &error, &debug);
            if (debug) api().g_free(debug);
            api().gst_mini_object_unref(reinterpret_cast<GstMiniObject*>(message));
            throw std::runtime_error(take_error(error, "GStreamer pipeline failed"));
        }
    }
    bool pull(GstClockTime timeout, Callback callback, void* user) {
        errors();
        Sample sample{api().gst_app_sink_try_pull_sample(reinterpret_cast<GstAppSink*>(sink.value), timeout)};
        if (!sample.value) { errors(); return false; }
        auto* buffer = api().gst_sample_get_buffer(sample.value);
        auto* caps = api().gst_sample_get_caps(sample.value);
        GstVideoInfo info{};
        if (!buffer || !caps || !api().gst_video_info_from_caps(&info, caps))
            throw std::runtime_error("Invalid decoded video sample");
        if (info.width <= 0 || info.height <= 0 || info.width > 8192 || info.height > 8192)
            throw std::runtime_error("Decoded video dimensions exceed limit");
        GstVideoFrame frame{};
        if (!api().gst_video_frame_map(&frame, &info, buffer, GST_MAP_READ))
            throw std::runtime_error("Cannot map decoded video frame");
        try {
            auto stride = frame.info.stride[0];
            if (std::abs(static_cast<int64_t>(stride)) < static_cast<int64_t>(info.width) * 4)
                throw std::runtime_error("Invalid decoded video stride");
            std::vector<uint8_t> rgba(static_cast<size_t>(info.width) * info.height * 4);
            auto* base = static_cast<uint8_t*>(frame.data[0]);
            for (int y = 0; y < info.height; ++y)
                memcpy(rgba.data() + static_cast<size_t>(y) * info.width * 4,
                    base + static_cast<ptrdiff_t>(y) * stride, static_cast<size_t>(info.width) * 4);
            int64_t pts = GST_BUFFER_PTS_IS_VALID(buffer) ? static_cast<int64_t>(GST_BUFFER_PTS(buffer) / 1000) : 0;
            callback(user, rgba.data(), rgba.size(), info.width, info.height, pts);
        } catch (...) { api().gst_video_frame_unmap(&frame); throw; }
        api().gst_video_frame_unmap(&frame);
        return true;
    }
    bool eos() { return api().gst_app_sink_is_eos(reinterpret_cast<GstAppSink*>(sink.value)); }
};
static const char* output_description = " ! videoconvert ! video/x-raw,format=RGBA ! appsink name=output sync=false max-buffers=4 drop=false wait-on-eos=false";
static std::string decoder_description(int mode) {
    initialize();
    return std::string("appsrc name=source format=time is-live=false block=false max-buffers=4 caps=\"video/x-h264,stream-format=byte-stream,alignment=au\" ! h264parse ! ")
        + decoder_name(mode) + output_description;
}
struct Decoder {
    std::unique_ptr<Pipeline> pipeline;
    int mode;
    bool hardware = false;
    explicit Decoder(int acceleration = 1) : mode(acceleration) { reset(); }
    void reset() {
        pipeline.reset();
        pipeline = std::make_unique<Pipeline>(decoder_description(mode));
        auto name = std::string(decoder_name(mode));
        hardware = name != "avdec_h264" && name != "openh264dec";
        pipeline->start();
    }
    void push(const uint8_t* data, size_t len, int64_t pts, bool key, Callback callback, void* user) {
        if (!len || len > 16 * 1024 * 1024 || pts < 0 || static_cast<uint64_t>(pts) > (UINT64_MAX - 1) / 1000)
            throw std::runtime_error("Invalid H.264 packet or timestamp");
        while (pipeline->pull(0, callback, user)) {}
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        auto* source = reinterpret_cast<GstAppSrc*>(pipeline->source.value);
        while (api().gst_app_src_get_current_level_buffers(source) >= 4) {
            pipeline->pull(GST_MSECOND, callback, user);
            if (std::chrono::steady_clock::now() >= deadline) throw std::runtime_error("H.264 decoder input timed out");
        }
        auto* buffer = api().gst_buffer_new_allocate(nullptr, len, nullptr);
        if (!buffer) throw std::bad_alloc();
        api().gst_buffer_fill(buffer, 0, data, len);
        GST_BUFFER_PTS(buffer) = static_cast<GstClockTime>(pts) * 1000;
        if (!key) GST_BUFFER_FLAG_SET(buffer, GST_BUFFER_FLAG_DELTA_UNIT);
        // push_buffer takes ownership, including on failure.
        if (api().gst_app_src_push_buffer(source, buffer) != GST_FLOW_OK) {
            pipeline->errors();
            throw std::runtime_error("H.264 decoder rejected input");
        }
        pipeline->pull(GST_MSECOND, callback, user);
        while (pipeline->pull(0, callback, user)) {}
    }
    void drain(Callback callback, void* user) {
        auto* source = reinterpret_cast<GstAppSrc*>(pipeline->source.value);
        if (api().gst_app_src_end_of_stream(source) != GST_FLOW_OK) throw std::runtime_error("Cannot drain H.264 decoder");
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        for (;;) {
            if (pipeline->pull(10 * GST_MSECOND, callback, user)) continue;
            if (pipeline->eos()) break;
            if (std::chrono::steady_clock::now() >= deadline) throw std::runtime_error("H.264 decoder drain timed out");
        }
    }
};
struct Source {
    Pipeline pipeline;
    explicit Source(const char* path) : pipeline(std::string("uridecodebin name=source caps=video/x-raw force-sw-decoders=true ! queue max-size-buffers=2 max-size-bytes=0 max-size-time=0") + output_description) {
        GError* error = nullptr;
        gchar* uri = api().g_filename_to_uri(path, nullptr, &error);
        if (!uri) throw std::runtime_error(take_error(error, "Video path must be absolute"));
        api().g_object_set(pipeline.source.value, "uri", uri, nullptr);
        api().g_free(uri);
        pipeline.start();
    }
    void next(Callback callback, void* user) {
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        for (;;) {
            if (pipeline.pull(10 * GST_MSECOND, callback, user)) return;
            if (pipeline.eos()) return;
            if (std::chrono::steady_clock::now() >= deadline) throw std::runtime_error("Video file decoding timed out");
        }
    }
};
// Probe audio independently so a video-only file does not fail delayed linking.
struct AudioApi {
    Library discover{"libgstpbutils-1.0.so.0"};
    decltype(&gst_discoverer_new) create = discover.get<decltype(&gst_discoverer_new)>("gst_discoverer_new");
    decltype(&gst_discoverer_discover_uri) probe = discover.get<decltype(&gst_discoverer_discover_uri)>("gst_discoverer_discover_uri");
    decltype(&gst_discoverer_info_get_audio_streams) streams = discover.get<decltype(&gst_discoverer_info_get_audio_streams)>("gst_discoverer_info_get_audio_streams");
    decltype(&gst_discoverer_info_get_result) result = discover.get<decltype(&gst_discoverer_info_get_result)>("gst_discoverer_info_get_result");
    decltype(&gst_discoverer_stream_info_list_free) free_streams = discover.get<decltype(&gst_discoverer_stream_info_list_free)>("gst_discoverer_stream_info_list_free");
    AudioApi() { discover.keep = true; }
};
struct AudioSource {
    std::unique_ptr<Pipeline> pipeline;
    explicit AudioSource(const char* path) {
        initialize();
        static AudioApi audio;
        GError* error = nullptr;
        std::unique_ptr<gchar, decltype(api().g_free)> uri(api().g_filename_to_uri(path, nullptr, &error), api().g_free);
        if (!uri) throw std::runtime_error(take_error(error, "Audio path must be absolute"));
        auto* discoverer = audio.create(10 * GST_SECOND, &error);
        if (!discoverer) throw std::runtime_error(take_error(error, "Cannot create audio discovery"));
        auto* info = audio.probe(discoverer, uri.get(), &error);
        api().g_object_unref(discoverer);
        if (!info) throw std::runtime_error(take_error(error, "Audio discovery failed"));
        auto result = audio.result(info);
        auto* streams = audio.streams(info);
        bool available = streams != nullptr;
        audio.free_streams(streams);
        api().g_object_unref(info);
        if (error || result != GST_DISCOVERER_OK) throw std::runtime_error(take_error(error, "Install GStreamer plugins for this audio format"));
        if (!available) return;
        pipeline = std::make_unique<Pipeline>("uridecodebin name=source caps=audio/x-raw force-sw-decoders=true ! queue max-size-buffers=2 max-size-bytes=0 max-size-time=0 ! audioconvert ! audioresample ! audio/x-raw,format=F32LE,rate=48000,channels=2,layout=interleaved ! appsink name=output sync=false max-buffers=4 drop=false wait-on-eos=false");
        api().g_object_set(pipeline->source.value, "uri", uri.get(), nullptr);
        pipeline->start();
    }
    void next(Callback callback, void* user) {
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        for (;;) {
            pipeline->errors();
            Sample sample{api().gst_app_sink_try_pull_sample(reinterpret_cast<GstAppSink*>(pipeline->sink.value), 10 * GST_MSECOND)};
            if (sample.value) {
                auto* buffer = api().gst_sample_get_buffer(sample.value);
                if (!buffer || !GST_BUFFER_PTS_IS_VALID(buffer)) throw std::runtime_error("Invalid decoded audio timestamp");
                size_t size = api().gst_buffer_get_size(buffer);
                if (size % 8 || size > 48000 * 8 * 10) throw std::runtime_error("Invalid decoded audio size");
                if (!size) continue;
                std::vector<uint8_t> pcm(size);
                if (api().gst_buffer_extract(buffer, 0, pcm.data(), size) != size) throw std::runtime_error("Cannot copy decoded audio");
                callback(user, pcm.data(), size, 48000, 2, static_cast<int64_t>(GST_BUFFER_PTS(buffer) / 1000));
                return;
            }
            pipeline->errors();
            if (pipeline->eos()) return;
            if (std::chrono::steady_clock::now() >= deadline) throw std::runtime_error("Audio file decoding timed out");
        }
    }
};
extern "C" {
const char* kg_error() { return last_error.c_str(); }
int kg_capabilities() {
    int flags = 0;
    if (protect([&] {
        initialize();
        if (!factory("videoconvert") || !factory("appsink")) throw std::runtime_error("Install GStreamer base plugins");
        if (factory("uridecodebin") && factory("queue")) flags |= 2;
        if (factory("appsrc") && factory("h264parse")) { decoder_name(); flags |= 1; }
    }) < 0) return flags;
    if (flags != 3) last_error = "Some GStreamer plugins are missing (playback, app, or h264parse)";
    return flags;
}
int kg_decoder_create(void** value) { return protect([&] { *value = new Decoder(); }); }
void kg_decoder_destroy(void* value) { delete static_cast<Decoder*>(value); }
int kg_decoder_push(void* value, const uint8_t* data, size_t len, int64_t pts, bool key, Callback callback, void* user) {
    return protect([&] { static_cast<Decoder*>(value)->push(data, len, pts, key, callback, user); });
}
int kg_decoder_drain(void* value, Callback callback, void* user) { return protect([&] { static_cast<Decoder*>(value)->drain(callback, user); }); }
int kg_decoder_reset(void* value) { return protect([&] { static_cast<Decoder*>(value)->reset(); }); }
int kg_source_create(const char* path, void** value) { return protect([&] { *value = new Source(path); }); }
int kg_source_next(void* value, Callback callback, void* user) { return protect([&] { static_cast<Source*>(value)->next(callback, user); }); }
void kg_source_destroy(void* value) { delete static_cast<Source*>(value); }
}

extern "C" int kg_audio_create(const char* path, void** value) {
    return protect([&] { auto source = std::make_unique<AudioSource>(path); *value = source->pipeline ? source.release() : nullptr; });
}
extern "C" int kg_audio_next(void* value, Callback cb, void* user) { return protect([&] { static_cast<AudioSource*>(value)->next(cb, user); }); }
extern "C" void kg_audio_destroy(void* value) { delete static_cast<AudioSource*>(value); }
#include "linux_encoder.h"

extern "C" int kg_decoder_create_mode(int mode, void** out) { return protect([&] { *out = new Decoder(mode); }); }
extern "C" bool kg_decoder_hardware(void* value) { return static_cast<Decoder*>(value)->hardware; }
