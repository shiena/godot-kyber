// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// Byte-buffer MediaCodec encoding. Surface input is a separate GPU-sharing path.
using PacketCallback = void (*)(void*, const uint8_t*, size_t, int64_t, bool);
struct Encoder {
    AMediaCodec* codec = nullptr;
    uint32_t width, height;
    int color = 21;
    bool started = false;
    std::vector<uint8_t> config;
    Encoder(uint32_t w, uint32_t h, uint32_t fps, uint32_t bitrate, int mode) : width(w), height(h) {
        if (mode == 2) throw std::runtime_error("Hardware-required encoding is unavailable: the NDK backend cannot verify codec acceleration on all supported Android versions; use Auto");
        media_status_t status = AMEDIA_ERROR_UNSUPPORTED;
        for (const char* name : {"", "c2.android.avc.encoder", "OMX.google.h264.encoder"}) {
            if (mode == 1 && !*name) continue;
            for (int format : {21, 19}) {
                codec = *name ? AMediaCodec_createCodecByName(name) : AMediaCodec_createEncoderByType("video/avc");
                if (!codec) continue;
                Format settings(AMediaFormat_new(), AMediaFormat_delete);
                AMediaFormat_setString(settings.get(), AMEDIAFORMAT_KEY_MIME, "video/avc");
                AMediaFormat_setInt32(settings.get(), AMEDIAFORMAT_KEY_WIDTH, w);
                AMediaFormat_setInt32(settings.get(), AMEDIAFORMAT_KEY_HEIGHT, h);
                AMediaFormat_setInt32(settings.get(), AMEDIAFORMAT_KEY_BIT_RATE, bitrate);
                AMediaFormat_setInt32(settings.get(), AMEDIAFORMAT_KEY_FRAME_RATE, fps);
                AMediaFormat_setInt32(settings.get(), AMEDIAFORMAT_KEY_I_FRAME_INTERVAL, 0);
                AMediaFormat_setInt32(settings.get(), AMEDIAFORMAT_KEY_COLOR_FORMAT, format);
                AMediaFormat_setInt32(settings.get(), "profile", 1);
                AMediaFormat_setInt32(settings.get(), "max-bframes", 0);
                AMediaFormat_setInt32(settings.get(), "color-standard", 4);
                AMediaFormat_setInt32(settings.get(), "color-range", 2);
                status = AMediaCodec_configure(codec, settings.get(), nullptr, nullptr, AMEDIACODEC_CONFIGURE_FLAG_ENCODE);
                if (status == AMEDIA_OK) status = AMediaCodec_start(codec);
                if (status == AMEDIA_OK) { started = true; color = format; return; }
                AMediaCodec_delete(codec); codec = nullptr;
            }
        }
        check(status);
    }
    ~Encoder() { if (codec) { if (started) AMediaCodec_stop(codec); AMediaCodec_delete(codec); } }
    void push(const uint8_t* rgba, size_t len, int64_t pts, PacketCallback cb, void* user) {
        if (len != size_t(width) * height * 4 || pts < 0) throw std::runtime_error("Invalid encoder frame");
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        ssize_t index;
        while ((index = AMediaCodec_dequeueInputBuffer(codec, 10000)) < 0) {
            if (std::chrono::steady_clock::now() >= deadline) throw std::runtime_error("Encoder input timed out");
        }
        size_t capacity = 0; auto* yuv = AMediaCodec_getInputBuffer(codec, index, &capacity);
        size_t size = size_t(width) * height * 3 / 2;
        if (!yuv || capacity < size) throw std::runtime_error("Encoder input buffer is too small");
        auto clamp = [](int v) { return static_cast<uint8_t>(std::max(0, std::min(255, v))); };
        for (uint32_t y = 0; y < height; ++y) for (uint32_t x = 0; x < width; ++x) {
            auto p = rgba + (size_t(y) * width + x) * 4;
            yuv[y * width + x] = clamp(((66 * p[0] + 129 * p[1] + 25 * p[2] + 128) >> 8) + 16);
        }
        for (uint32_t y = 0; y < height; y += 2) for (uint32_t x = 0; x < width; x += 2) {
            int r = 0, g = 0, b = 0;
            for (uint32_t dy = 0; dy < 2; ++dy) for (uint32_t dx = 0; dx < 2; ++dx) {
                auto p = rgba + (size_t(y + dy) * width + x + dx) * 4;
                r += p[0]; g += p[1]; b += p[2];
            }
            r /= 4; g /= 4; b /= 4;
            size_t plane = size_t(width) * height, uv = (y / 2) * (width / 2) + x / 2;
            yuv[plane + uv * (color == 21 ? 2 : 1)] = clamp(((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128);
            yuv[color == 21 ? plane + uv * 2 + 1 : plane + plane / 4 + uv] = clamp(((112 * r - 94 * g - 18 * b + 128) >> 8) + 128);
        }
        check(AMediaCodec_queueInputBuffer(codec, index, 0, size, pts, 0));
        for (;;) {
            AMediaCodecBufferInfo info{};
            auto out = AMediaCodec_dequeueOutputBuffer(codec, &info, 10000);
            if (out >= 0) {
                size_t available = 0; auto* bytes = AMediaCodec_getOutputBuffer(codec, out, &available);
                try {
                    if (!bytes || info.offset < 0 || info.size < 0 || size_t(info.offset) + info.size > available) throw std::runtime_error("Invalid encoded buffer");
                    bytes += info.offset;
                    if (info.flags & AMEDIACODEC_BUFFER_FLAG_CODEC_CONFIG) config.assign(bytes, bytes + info.size);
                    else if (info.size) {
                        std::vector<uint8_t> packet = config;
                        packet.insert(packet.end(), bytes, bytes + info.size);
                        cb(user, packet.data(), packet.size(), info.presentationTimeUs, (info.flags & 1) != 0);
                        check(AMediaCodec_releaseOutputBuffer(codec, out, false));
                        return;
                    }
                } catch (...) { AMediaCodec_releaseOutputBuffer(codec, out, false); throw; }
                check(AMediaCodec_releaseOutputBuffer(codec, out, false));
            }
            if (std::chrono::steady_clock::now() >= deadline) throw std::runtime_error("Encoder output timed out");
        }
    }
};
extern "C" {
int kg_encoder_create(uint32_t w, uint32_t h, uint32_t fps, uint32_t bitrate, int mode, void** out) {
    return protect([&] { *out = new Encoder(w, h, fps, bitrate, mode); });
}
void kg_encoder_destroy(void* value) { delete static_cast<Encoder*>(value); }
bool kg_encoder_hardware(void*) { return false; } // Auto uses the platform selection; acceleration is unverified.
int kg_encoder_push(void* value, const uint8_t* rgba, size_t len, int64_t pts, PacketCallback cb, void* user) {
    return protect([&] { static_cast<Encoder*>(value)->push(rgba, len, pts, cb, user); });
}
int kg_encoder_drain(void*, PacketCallback, void*) { return 0; } // Each input is fully consumed synchronously.
}
