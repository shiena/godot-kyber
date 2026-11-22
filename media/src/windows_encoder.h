// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// Media Foundation H.264 encoders. Included after the common Windows helpers.
#include <chrono>
#include <thread>
using PacketCallback = void (*)(void*, const uint8_t*, size_t, int64_t, bool);
struct CodecWait {
    HANDLE timer = CreateWaitableTimerExW(nullptr, nullptr, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS);
    ~CodecWait() { if (timer) CloseHandle(timer); }
    void sleep() {
        if (!timer) { std::this_thread::sleep_for(std::chrono::milliseconds(1)); return; }
        LARGE_INTEGER due; due.QuadPart = -10000;
        if (!SetWaitableTimer(timer, &due, 0, nullptr, nullptr, FALSE)) throw Failure{HRESULT_FROM_WIN32(GetLastError())};
        if (WaitForSingleObject(timer, 5000) != WAIT_OBJECT_0) throw Failure{HRESULT_FROM_WIN32(WAIT_TIMEOUT)};
    }
};

struct Encoder {
    Runtime runtime;
    ComPtr<IMFTransform> transform;
    ComPtr<IMFMediaEventGenerator> events;
    ComPtr<ICodecAPI> codec;
    UINT width, height, fps;
    bool hardware = false;
    unsigned inputs = 0;
    std::vector<uint8_t> configuration;
    CodecWait timer;
    Encoder(UINT w, UINT h, UINT rate, UINT bitrate, int acceleration) : width(w), height(h), fps(rate) {
        if (!w || !h || w > 4096 || h > 4096 || w % 2 || h % 2 || !rate || rate > 120) throw Failure{E_INVALIDARG};
        if (acceleration != 1) {
            MFT_REGISTER_TYPE_INFO in{MFMediaType_Video, MFVideoFormat_NV12};
            MFT_REGISTER_TYPE_INFO out{MFMediaType_Video, MFVideoFormat_H264};
            IMFActivate** list = nullptr;
            UINT count = 0;
            HRESULT hr = media_api().enumerate(MFT_CATEGORY_VIDEO_ENCODER,
                MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER, &in, &out, &list, &count);
            if (SUCCEEDED(hr)) {
                for (UINT i = 0; i < count; ++i) {
                    if (!transform) {
                        hr = list[i]->ActivateObject(IID_PPV_ARGS(&transform));
                        if (SUCCEEDED(hr)) {
                            hr = protect([&] { configure(bitrate); });
                            if (FAILED(hr)) { shutdown(); transform.Reset(); events.Reset(); codec.Reset(); }
                            else hardware = true;
                        }
                    }
                    list[i]->Release();
                }
                CoTaskMemFree(list);
            }
            if (!transform && acceleration == 2) throw Failure{FAILED(hr) ? hr : MF_E_TOPO_CODEC_NOT_FOUND};
        }
        if (!transform) {
            check(CoCreateInstance(CLSID_CMSH264EncoderMFT, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&transform)));
            configure(bitrate);
        }
        check(transform->ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0));
        check(transform->ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0));
    }
    void shutdown() noexcept {
        ComPtr<IMFShutdown> stop;
        if (transform && SUCCEEDED(transform.As(&stop))) stop->Shutdown();
    }
    ~Encoder() { shutdown(); }
    void property(const GUID& id, ULONG value) {
        VARIANT v; VariantInit(&v); v.vt = VT_UI4; v.ulVal = value;
        check(codec->SetValue(&id, &v));
    }
    void configure(UINT bitrate) {
        ComPtr<IMFAttributes> attributes;
        check(transform->GetAttributes(&attributes));
        if (MFGetAttributeUINT32(attributes.Get(), MF_TRANSFORM_ASYNC, FALSE)) {
            check(attributes->SetUINT32(MF_TRANSFORM_ASYNC_UNLOCK, TRUE));
            check(transform.As(&events));
        }
        check(transform.As(&codec));
        VARIANT low; VariantInit(&low); low.vt = VT_BOOL; low.boolVal = VARIANT_TRUE;
        codec->SetValue(&CODECAPI_AVLowLatencyMode, &low);
        // Every access unit is independently decodable, including after loss or a late join.
        property(CODECAPI_AVEncMPVGOPSize, 1);
        // Baseline profile excludes B pictures. Some hardware MFTs reject this property.
        auto out = video_type(MFVideoFormat_H264);
        check(out->SetUINT32(MF_MT_AVG_BITRATE, bitrate));
        check(out->SetUINT32(MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive));
        check(out->SetUINT32(MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Base));
        check(MFSetAttributeSize(out.Get(), MF_MT_FRAME_SIZE, width, height));
        check(MFSetAttributeRatio(out.Get(), MF_MT_FRAME_RATE, fps, 1));
        check(MFSetAttributeRatio(out.Get(), MF_MT_PIXEL_ASPECT_RATIO, 1, 1));
        check(transform->SetOutputType(0, out.Get(), 0));
        auto in = video_type(MFVideoFormat_NV12);
        check(in->SetUINT32(MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive));
        check(MFSetAttributeSize(in.Get(), MF_MT_FRAME_SIZE, width, height));
        check(MFSetAttributeRatio(in.Get(), MF_MT_FRAME_RATE, fps, 1));
        check(MFSetAttributeRatio(in.Get(), MF_MT_PIXEL_ASPECT_RATIO, 1, 1));
        check(transform->SetInputType(0, in.Get(), 0));
    }
    bool receive(PacketCallback cb, void* user) {
        ComPtr<IMFSample> sample;
        HRESULT hr = output(transform.Get(), sample);
        if (hr == MF_E_TRANSFORM_NEED_MORE_INPUT) return false;
        check(hr);
        if (!sample) return false;
        ComPtr<IMFMediaType> type;
        check(transform->GetOutputCurrentType(0, &type));
        UINT size = 0;
        if (SUCCEEDED(type->GetBlobSize(MF_MT_MPEG_SEQUENCE_HEADER, &size)) && size < 65536) {
            configuration.resize(size);
            check(type->GetBlob(MF_MT_MPEG_SEQUENCE_HEADER, configuration.data(), size, nullptr));
        }
        ComPtr<IMFMediaBuffer> buffer;
        check(sample->ConvertToContiguousBuffer(&buffer));
        DWORD length; BYTE* data;
        check(buffer->Lock(&data, nullptr, &length));
        try {
            std::vector<uint8_t> packet = configuration;
            packet.insert(packet.end(), data, data + length);
            LONGLONG pts = 0; check(sample->GetSampleTime(&pts));
            bool key = MFGetAttributeUINT32(sample.Get(), MFSampleExtension_CleanPoint, FALSE) != 0;
            cb(user, packet.data(), packet.size(), pts / 10, key);
        } catch (...) { buffer->Unlock(); throw; }
        check(buffer->Unlock());
        return true;
    }
    MediaEventType event() {
        ComPtr<IMFMediaEvent> event;
        HRESULT hr = events->GetEvent(MF_EVENT_FLAG_NO_WAIT, &event);
        if (hr == MF_E_NO_EVENTS_AVAILABLE) return MEUnknown;
        check(hr);
        HRESULT status; check(event->GetStatus(&status)); check(status);
        MediaEventType type; check(event->GetType(&type));
        if (type == METransformNeedInput) ++inputs;
        return type;
    }
    void push(const uint8_t* rgba, size_t len, int64_t pts, PacketCallback cb, void* user) {
        if (len != static_cast<size_t>(width) * height * 4 || pts < 0 || pts > INT64_MAX / 10) throw Failure{E_INVALIDARG};
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        auto wait = [&] {
            if (std::chrono::steady_clock::now() >= deadline) throw Failure{HRESULT_FROM_WIN32(WAIT_TIMEOUT)};
            timer.sleep();
        };
        while (events && !inputs) {
            auto type = event();
            if (type == METransformHaveOutput) receive(cb, user);
            if (type == MEUnknown) wait();
        }
        auto sample = sample_buffer(width * height * 3 / 2);
        ComPtr<IMFMediaBuffer> buffer; check(sample->GetBufferByIndex(0, &buffer));
        BYTE* nv12; check(buffer->Lock(&nv12, nullptr, nullptr));
        auto clamp = [](int value) { return static_cast<uint8_t>(std::max(0, std::min(255, value))); };
        // BT.601 limited-range NV12. Chroma is averaged over each 2x2 block.
        for (UINT y = 0; y < height; ++y) for (UINT x = 0; x < width; ++x) {
            auto p = rgba + (static_cast<size_t>(y) * width + x) * 4;
            nv12[y * width + x] = clamp(((66 * p[0] + 129 * p[1] + 25 * p[2] + 128) >> 8) + 16);
        }
        for (UINT y = 0; y < height; y += 2) for (UINT x = 0; x < width; x += 2) {
            int r = 0, g = 0, b = 0;
            for (UINT dy = 0; dy < 2; ++dy) for (UINT dx = 0; dx < 2; ++dx) {
                auto p = rgba + (static_cast<size_t>(y + dy) * width + x + dx) * 4;
                r += p[0]; g += p[1]; b += p[2];
            }
            r /= 4; g /= 4; b /= 4;
            size_t uv = static_cast<size_t>(width) * height + (y / 2) * width + x;
            nv12[uv] = clamp(((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128);
            nv12[uv + 1] = clamp(((112 * r - 94 * g - 18 * b + 128) >> 8) + 128);
        }
        check(buffer->Unlock()); check(buffer->SetCurrentLength(width * height * 3 / 2));
        check(sample->SetSampleTime(pts * 10)); check(sample->SetSampleDuration(10000000 / fps));
        check(transform->ProcessInput(0, sample.Get(), 0));
        if (events) {
            --inputs;
            for (;;) {
                auto type = event();
                if (type == METransformHaveOutput && receive(cb, user)) break;
                if (type == MEUnknown) wait();
            }
        } else { while (receive(cb, user)) {} }
    }
    void drain(PacketCallback cb, void* user) {
        check(transform->ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0));
        check(transform->ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0));
        if (!events) { while (receive(cb, user)) {} return; }
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        for (;;) {
            auto type = event();
            if (type == METransformDrainComplete) return;
            if (type == METransformHaveOutput) receive(cb, user);
            if (std::chrono::steady_clock::now() >= deadline) throw Failure{HRESULT_FROM_WIN32(WAIT_TIMEOUT)};
            if (type == MEUnknown) timer.sleep();
        }
    }
};
extern "C" {
HRESULT km_encoder_create(UINT w, UINT h, UINT fps, UINT bitrate, int mode, void** result) {
    return protect([&] { *result = new Encoder(w, h, fps, bitrate, mode); });
}
void km_encoder_destroy(void* value) { delete static_cast<Encoder*>(value); }
bool km_encoder_hardware(void* value) { return static_cast<Encoder*>(value)->hardware; }
HRESULT km_encoder_push(void* value, const uint8_t* rgba, size_t len, int64_t pts, PacketCallback cb, void* user) {
    return protect([&] { static_cast<Encoder*>(value)->push(rgba, len, pts, cb, user); });
}
HRESULT km_encoder_drain(void* value, PacketCallback cb, void* user) {
    return protect([&] { static_cast<Encoder*>(value)->drain(cb, user); });
}
}
