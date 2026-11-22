// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// Android system media services. Codec output is copied from YUV ImageReader planes.
#include <media/NdkMediaCodec.h>
#include <media/NdkMediaExtractor.h>
#include <media/NdkImageReader.h>
#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstring>
#include <memory>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

using Callback = void (*)(void*, const uint8_t*, size_t, uint32_t, uint32_t, int64_t);
static thread_local std::string last_error;
static void check(media_status_t code) {
    if (code != AMEDIA_OK) throw std::runtime_error("Android media error: " + std::to_string(code));
}
template<class F> static int protect(F fn) noexcept {
    try { fn(); last_error.clear(); return 0; }
    catch (const std::exception& e) { last_error = e.what(); return -1; }
    catch (...) { last_error = "Unknown Android media failure"; return -1; }
}
using Format = std::unique_ptr<AMediaFormat, decltype(&AMediaFormat_delete)>;
struct Codec {
    AMediaCodec* codec = nullptr;
    AImageReader* reader = nullptr;
    bool started = false, input_eos = false, output_eos = false;
    int32_t standard = 4, range = 2;
    ~Codec() {
        if (codec) { if (started) AMediaCodec_stop(codec); AMediaCodec_delete(codec); }
        if (reader) AImageReader_delete(reader);
    }
    void start(AMediaFormat* format, int mode = 0) {
        int32_t width = 0, height = 0;
        const char* mime = nullptr;
        AMediaFormat_getInt32(format, AMEDIAFORMAT_KEY_WIDTH, &width);
        AMediaFormat_getInt32(format, AMEDIAFORMAT_KEY_HEIGHT, &height);
        if (!AMediaFormat_getString(format, AMEDIAFORMAT_KEY_MIME, &mime) ||
            width <= 0 || height <= 0 || width > 8192 || height > 8192)
            throw std::runtime_error("Invalid video format");
        if (mode == 2) throw std::runtime_error("Hardware-required decoding is unavailable: this NDK backend cannot verify acceleration on all supported Android versions; use Auto");
        if (mode == 1) {
            codec = AMediaCodec_createCodecByName("c2.android.avc.decoder");
            if (!codec) codec = AMediaCodec_createCodecByName("OMX.google.h264.decoder");
        } else codec = AMediaCodec_createDecoderByType(mime);
        if (!codec) throw std::runtime_error("No Android decoder for " + std::string(mime));
        check(AImageReader_new(width, height, AIMAGE_FORMAT_YUV_420_888, 4, &reader));
        ANativeWindow* window = nullptr;
        check(AImageReader_getWindow(reader, &window));
        // COLOR_FormatYUV420Flexible prevents opaque vendor output on CPU surfaces.
        AMediaFormat_setInt32(format, AMEDIAFORMAT_KEY_COLOR_FORMAT, 0x7f420888);
        auto status = AMediaCodec_configure(codec, format, window, nullptr, 0);
        // Platform software codecs accept dimensions below hardware decoder limits.
        if (status != AMEDIA_OK && !strcmp(mime, "video/avc")) {
            for (const char* name : {"c2.android.avc.decoder", "OMX.google.h264.decoder"}) {
                if (codec) AMediaCodec_delete(codec);
                codec = AMediaCodec_createCodecByName(name);
                if (!codec) continue;
                status = AMediaCodec_configure(codec, format, window, nullptr, 0);
                if (status == AMEDIA_OK) break;
            }
        }
        check(status);
        check(AMediaCodec_start(codec));
        started = true;
    }
    void emit(Callback callback, void* user, int64_t pts) {
        AImage* raw = nullptr;
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(3);
        for (;;) {
            auto status = AImageReader_acquireNextImage(reader, &raw);
            if (status == AMEDIA_OK) break;
            if (status != AMEDIA_IMGREADER_NO_BUFFER_AVAILABLE) check(status);
            if (std::chrono::steady_clock::now() >= deadline)
                throw std::runtime_error("Timed out waiting for decoded Android image");
            std::this_thread::sleep_for(std::chrono::milliseconds(1));
        }
        std::unique_ptr<AImage, decltype(&AImage_delete)> image(raw, AImage_delete);
        AImageCropRect crop{};
        check(AImage_getCropRect(raw, &crop));
        const int w = crop.right - crop.left, h = crop.bottom - crop.top;
        if (w <= 0 || h <= 0 || w > 8192 || h > 8192 || crop.left < 0 || crop.top < 0)
            throw std::runtime_error("Invalid Android image crop");
        uint8_t* planes[3]{};
        int lengths[3]{}, rows[3]{}, pixels[3]{};
        for (int p = 0; p < 3; ++p) {
            check(AImage_getPlaneData(raw, p, &planes[p], &lengths[p]));
            check(AImage_getPlaneRowStride(raw, p, &rows[p]));
            check(AImage_getPlanePixelStride(raw, p, &pixels[p]));
            if (rows[p] <= 0 || pixels[p] <= 0) throw std::runtime_error("Invalid image stride for plane " + std::to_string(p) + ": " + std::to_string(rows[p]) + "/" + std::to_string(pixels[p]));
        }
        auto sample = [&](int p, int x, int y) -> float {
            if (p) { x /= 2; y /= 2; }
            const int64_t offset = int64_t(y) * rows[p] + int64_t(x) * pixels[p];
            if (offset < 0 || offset >= lengths[p]) throw std::runtime_error("Invalid image plane");
            return planes[p][offset];
        };
        std::vector<uint8_t> rgba(size_t(w) * h * 4);
        const float kr = standard == 1 ? 0.2126f : 0.299f;
        const float kb = standard == 1 ? 0.0722f : 0.114f;
        auto byte = [](float x) { return uint8_t(std::clamp(x + 0.5f, 0.0f, 255.0f)); };
        for (int y = 0; y < h; ++y) for (int x = 0; x < w; ++x) {
            const int sx = x + crop.left, sy = y + crop.top;
            float luma = sample(0, sx, sy), u = sample(1, sx, sy) - 128, v = sample(2, sx, sy) - 128;
            if (range != 1) { luma = (luma - 16) * (255.0f / 219); u *= 255.0f / 224; v *= 255.0f / 224; }
            const auto i = (size_t(y) * w + x) * 4;
            rgba[i] = byte(luma + 2 * (1 - kr) * v);
            rgba[i + 1] = byte(luma - 2 * kb * (1 - kb) / (1 - kr - kb) * u - 2 * kr * (1 - kr) / (1 - kr - kb) * v);
            rgba[i + 2] = byte(luma + 2 * (1 - kb) * u);
            rgba[i + 3] = 255;
        }
        callback(user, rgba.data(), rgba.size(), w, h, pts);
    }
    bool output(Callback callback, void* user, int64_t timeout) {
        AMediaCodecBufferInfo info{};
        auto index = AMediaCodec_dequeueOutputBuffer(codec, &info, timeout);
        if (index == AMEDIACODEC_INFO_OUTPUT_FORMAT_CHANGED) {
            Format format(AMediaCodec_getOutputFormat(codec), AMediaFormat_delete);
            AMediaFormat_getInt32(format.get(), "color-standard", &standard);
            AMediaFormat_getInt32(format.get(), "color-range", &range);
            if (standard == 6) throw std::runtime_error("BT.2020/HDR video is not supported");
        } else if (index >= 0) {
            output_eos = (info.flags & AMEDIACODEC_BUFFER_FLAG_END_OF_STREAM) != 0;
            const bool frame = info.size > 0 && !(info.flags & AMEDIACODEC_BUFFER_FLAG_CODEC_CONFIG);
            check(AMediaCodec_releaseOutputBuffer(codec, index, frame));
            if (frame) { emit(callback, user, info.presentationTimeUs); return true; }
        } else if (index != AMEDIACODEC_INFO_TRY_AGAIN_LATER && index != AMEDIACODEC_INFO_OUTPUT_BUFFERS_CHANGED) {
            throw std::runtime_error("Android decoder output error: " + std::to_string(index));
        }
        return false;
    }
};
struct Source {
    AMediaExtractor* extractor = AMediaExtractor_new();
    Codec decoder;
    ~Source() { if (extractor) AMediaExtractor_delete(extractor); }
    void open(const char* path) {
        int fd = ::open(path, O_RDONLY | O_CLOEXEC);
        if (fd < 0) throw std::runtime_error("Cannot open video file");
        struct stat st{};
        auto status = fstat(fd, &st) == 0 ? AMediaExtractor_setDataSourceFd(extractor, fd, 0, st.st_size) : AMEDIA_ERROR_IO;
        close(fd);
        check(status);
        for (size_t i = 0; i < AMediaExtractor_getTrackCount(extractor); ++i) {
            Format format(AMediaExtractor_getTrackFormat(extractor, i), AMediaFormat_delete);
            const char* mime = nullptr;
            if (AMediaFormat_getString(format.get(), AMEDIAFORMAT_KEY_MIME, &mime) && !strncmp(mime, "video/", 6)) {
                check(AMediaExtractor_selectTrack(extractor, i));
                decoder.start(format.get());
                return;
            }
        }
        throw std::runtime_error("No video track in file");
    }
    void next(Callback callback, void* user) {
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        while (!decoder.output_eos) {
            if (!decoder.input_eos) {
                auto index = AMediaCodec_dequeueInputBuffer(decoder.codec, 0);
                if (index >= 0) {
                    size_t capacity = 0;
                    auto* buffer = AMediaCodec_getInputBuffer(decoder.codec, index, &capacity);
                    if (!buffer) throw std::runtime_error("Missing codec input buffer");
                    auto bytes = AMediaExtractor_readSampleData(extractor, buffer, capacity);
                    decoder.input_eos = bytes < 0;
                    check(AMediaCodec_queueInputBuffer(decoder.codec, index, 0, bytes < 0 ? 0 : bytes,
                        bytes < 0 ? 0 : AMediaExtractor_getSampleTime(extractor),
                        bytes < 0 ? AMEDIACODEC_BUFFER_FLAG_END_OF_STREAM : 0));
                    if (bytes >= 0) AMediaExtractor_advance(extractor);
                }
            }
            if (decoder.output(callback, user, 10000)) return;
            if (std::chrono::steady_clock::now() >= deadline) throw std::runtime_error("Android decoder stalled");
        }
    }
};
// Parse only bounded SPS syntax needed to initialize the Android surface.
struct Bits {
    std::vector<uint8_t> bytes;
    size_t position = 0;
    explicit Bits(const std::vector<uint8_t>& nal) {
        int zeros = 0;
        for (size_t i = 1; i < nal.size(); ++i) {
            if (zeros == 2 && nal[i] == 3) { zeros = 0; continue; }
            bytes.push_back(nal[i]); zeros = nal[i] == 0 ? zeros + 1 : 0;
        }
    }
    uint32_t read(unsigned n) {
        if (n > 32 || position + n > bytes.size() * 8) throw std::runtime_error("Truncated H.264 SPS");
        uint32_t value = 0;
        while (n--) { value = (value << 1) | ((bytes[position / 8] >> (7 - position % 8)) & 1); ++position; }
        return value;
    }
    uint32_t ue() {
        unsigned n = 0;
        while (!read(1)) if (++n > 24) throw std::runtime_error("Invalid H.264 Exp-Golomb value");
        return ((1u << n) - 1) + read(n);
    }
    int32_t se() { auto v = ue(); return v & 1 ? int32_t((v + 1) / 2) : -int32_t(v / 2); }
};
static std::pair<int, int> dimensions(const std::vector<uint8_t>& sps) {
    Bits b(sps);
    auto profile = b.read(8); b.read(16); b.ue();
    if (profile == 100 || profile == 110 || profile == 122 || profile == 244 || profile == 44 ||
        profile == 83 || profile == 86 || profile == 118 || profile == 128 || profile == 138 || profile == 139 || profile == 134 || profile == 135) {
        if (b.ue() != 1 || b.ue() != 0 || b.ue() != 0)
            throw std::runtime_error("Only 8-bit YUV420 H.264 is supported");
        b.read(1);
        if (b.read(1)) for (int i = 0; i < 8; ++i) if (b.read(1)) {
            int last = 8, next = 8;
            for (int j = 0; j < (i < 6 ? 16 : 64); ++j) {
                if (next) next = (last + b.se() + 256) % 256;
                if (next) last = next;
            }
        }
    }
    b.ue(); auto order = b.ue();
    if (order == 0) b.ue();
    else if (order == 1) {
        b.read(1); b.se(); b.se(); auto n = b.ue();
        if (n > 256) throw std::runtime_error("Invalid H.264 order cycle");
        while (n--) b.se();
    } else if (order != 2) throw std::runtime_error("Invalid H.264 order type");
    b.ue(); b.read(1);
    auto w = b.ue(), h = b.ue(); auto progressive = b.read(1);
    if (!progressive) b.read(1);
    b.read(1);
    uint32_t left = 0, right = 0, top = 0, bottom = 0;
    if (b.read(1)) { left = b.ue(); right = b.ue(); top = b.ue(); bottom = b.ue(); }
    int64_t width = (int64_t(w) + 1) * 16 - 2 * (int64_t(left) + right);
    int64_t height = (int64_t(h) + 1) * 16 * (2 - progressive) - 2 * (2 - progressive) * (int64_t(top) + bottom);
    if (width <= 0 || height <= 0 || width > 8192 || height > 8192) throw std::runtime_error("Invalid H.264 dimensions");
    return {int(width), int(height)};
}
struct Decoder {
    int mode = 0;
    explicit Decoder(int acceleration = 0) : mode(acceleration) {}
    std::unique_ptr<Codec> codec;
    std::vector<uint8_t> sps, pps;
    void send(const uint8_t* data, size_t len, int64_t pts, uint32_t flags, Callback callback, void* user) {
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        for (;;) {
            auto index = AMediaCodec_dequeueInputBuffer(codec->codec, 10000);
            if (index >= 0) {
                size_t capacity = 0;
                auto* buffer = AMediaCodec_getInputBuffer(codec->codec, index, &capacity);
                if (!buffer || len > capacity) throw std::runtime_error("H.264 access unit exceeds codec capacity");
                if (len) memcpy(buffer, data, len);
                check(AMediaCodec_queueInputBuffer(codec->codec, index, 0, len, pts, flags));
                return;
            }
            codec->output(callback, user, 0);
            if (std::chrono::steady_clock::now() >= deadline) throw std::runtime_error("Android input stalled");
        }
    }
    void drain(Callback callback, void* user) {
        if (!codec || codec->output_eos) return;
        if (!codec->input_eos) { send(nullptr, 0, 0, AMEDIACODEC_BUFFER_FLAG_END_OF_STREAM, callback, user); codec->input_eos = true; }
        const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        while (!codec->output_eos) {
            codec->output(callback, user, 10000);
            if (std::chrono::steady_clock::now() >= deadline) throw std::runtime_error("Android drain stalled");
        }
    }
    void push(const uint8_t* data, size_t len, int64_t pts, Callback callback, void* user) {
        auto new_sps = sps, new_pps = pps;
        bool slice = false;
        for (size_t i = 0; i + 3 < len;) {
            size_t prefix = 0;
            if (!data[i] && !data[i + 1] && data[i + 2] == 1) prefix = 3;
            else if (!data[i] && !data[i + 1] && !data[i + 2] && data[i + 3] == 1) prefix = 4;
            if (!prefix) { ++i; continue; }
            size_t start = i + prefix, end = start;
            while (end + 3 < len && !(data[end] == 0 && data[end + 1] == 0 &&
                (data[end + 2] == 1 || (data[end + 2] == 0 && data[end + 3] == 1)))) ++end;
            if (end + 3 >= len) end = len;
            if (start < end) {
                auto type = data[start] & 31;
                if (type == 7) new_sps.assign(data + start, data + end);
                if (type == 8) new_pps.assign(data + start, data + end);
                if (type == 1 || type == 5) slice = true;
            }
            i = end;
        }
        if (new_sps != sps || new_pps != pps) { drain(callback, user); codec.reset(); sps = new_sps; pps = new_pps; }
        if (!slice) return;
        if (!codec) {
            if (sps.empty() || pps.empty()) throw std::runtime_error("H.264 SPS/PPS required");
            auto [width, height] = dimensions(sps);
            Format format(AMediaFormat_new(), AMediaFormat_delete);
            AMediaFormat_setString(format.get(), AMEDIAFORMAT_KEY_MIME, "video/avc");
            AMediaFormat_setInt32(format.get(), AMEDIAFORMAT_KEY_WIDTH, width);
            AMediaFormat_setInt32(format.get(), AMEDIAFORMAT_KEY_HEIGHT, height);
            AMediaFormat_setInt32(format.get(), AMEDIAFORMAT_KEY_MAX_INPUT_SIZE, 16 * 1024 * 1024);
            auto csd0 = sps, csd1 = pps;
            csd0.insert(csd0.begin(), {0, 0, 0, 1}); csd1.insert(csd1.begin(), {0, 0, 0, 1});
            AMediaFormat_setBuffer(format.get(), "csd-0", csd0.data(), csd0.size());
            AMediaFormat_setBuffer(format.get(), "csd-1", csd1.data(), csd1.size());
            codec = std::make_unique<Codec>(); codec->start(format.get(), mode);
        }
        if (codec->input_eos) throw std::runtime_error("Reset decoder before reusing a drained stream");
        send(data, len, pts, 0, callback, user);
        // MediaCodec is asynchronous. Give the current frame a bounded chance
        // to finish instead of collecting several outputs on a later packet.
        // Batching outputs would discard intermediate frames in the live mailbox.
        codec->output(callback, user, 20000);
        while (codec->output(callback, user, 0)) {}
    }
};
struct AudioSource {
    AMediaExtractor* extractor = AMediaExtractor_new();
    AMediaCodec* codec = nullptr;
    bool started = false, input_eos = false, output_eos = false, available = false;
    int32_t rate = 0, channels = 0, encoding = 2;
    int32_t resample_rate = 0;
    int64_t origin = 0;
    uint64_t input_count = 0, output_count = 0;
    float previous[2] = {0, 0};
    ~AudioSource() {
        if (codec) { if (started) AMediaCodec_stop(codec); AMediaCodec_delete(codec); }
        if (extractor) AMediaExtractor_delete(extractor);
    }
    void format(AMediaFormat* value) {
        AMediaFormat_getInt32(value, AMEDIAFORMAT_KEY_SAMPLE_RATE, &rate);
        AMediaFormat_getInt32(value, AMEDIAFORMAT_KEY_CHANNEL_COUNT, &channels);
        AMediaFormat_getInt32(value, "pcm-encoding", &encoding);
        if (rate < 8000 || rate > 192000 || channels < 1 || channels > 2 || (encoding != 2 && encoding != 4))
            throw std::runtime_error("Android audio requires mono/stereo PCM16 or float PCM");
    }
    void open(const char* path) {
        if (!extractor) throw std::runtime_error("Cannot create audio extractor");
        int fd = ::open(path, O_RDONLY | O_CLOEXEC);
        if (fd < 0) throw std::runtime_error("Cannot open audio file");
        struct stat st{};
        auto status = fstat(fd, &st) == 0 ? AMediaExtractor_setDataSourceFd(extractor, fd, 0, st.st_size) : AMEDIA_ERROR_IO;
        close(fd); check(status);
        for (size_t i = 0; i < AMediaExtractor_getTrackCount(extractor); ++i) {
            Format value(AMediaExtractor_getTrackFormat(extractor, i), AMediaFormat_delete);
            const char* mime = nullptr;
            if (AMediaFormat_getString(value.get(), AMEDIAFORMAT_KEY_MIME, &mime) && !strncmp(mime, "audio/", 6)) {
                check(AMediaExtractor_selectTrack(extractor, i));
                format(value.get());
                codec = AMediaCodec_createDecoderByType(mime);
                if (!codec) throw std::runtime_error("No Android decoder for this audio format");
                check(AMediaCodec_configure(codec, value.get(), nullptr, nullptr, 0));
                check(AMediaCodec_start(codec));
                started = available = true;
                return;
            }
        }
    }
    bool emit(const uint8_t* data, size_t size, int64_t pts, Callback cb, void* user) {
        size_t bytes = encoding == 4 ? 4 : 2;
        if (size % (bytes * channels)) throw std::runtime_error("Invalid Android PCM alignment");
        size_t frames = size / (bytes * channels);
        if (!frames) return false;
        int64_t expected = origin + static_cast<int64_t>(input_count * 1000000 / std::max(resample_rate, 1));
        if (rate != resample_rate || std::abs(pts - expected) > 2000) {
            origin = pts; input_count = output_count = 0; resample_rate = rate;
        }
        uint64_t first = output_count;
        std::vector<float> pcm;
        pcm.reserve((frames * 48000 / rate + 2) * 2);
        for (size_t frame = 0; frame < frames; ++frame, ++input_count) {
            float current[2];
            for (int channel = 0; channel < 2; ++channel) {
                size_t offset = (frame * channels + std::min(channel, channels - 1)) * bytes;
                if (encoding == 4) memcpy(&current[channel], data + offset, 4);
                else { int16_t sample; memcpy(&sample, data + offset, 2); current[channel] = sample / 32768.0f; }
                if (!std::isfinite(current[channel])) throw std::runtime_error("Non-finite decoded audio sample");
            }
            if (!input_count) { previous[0] = current[0]; previous[1] = current[1]; }
            while (output_count * rate <= input_count * 48000) {
                double position = static_cast<double>(output_count * rate) / 48000;
                float fraction = input_count ? static_cast<float>(position - (input_count - 1)) : 1.0f;
                for (int channel = 0; channel < 2; ++channel)
                    pcm.push_back(previous[channel] + fraction * (current[channel] - previous[channel]));
                ++output_count;
            }
            previous[0] = current[0]; previous[1] = current[1];
        }
        if (!pcm.empty()) cb(user, reinterpret_cast<const uint8_t*>(pcm.data()), pcm.size() * 4, 48000, 2,
            origin + static_cast<int64_t>(first * 1000000 / 48000));
        return !pcm.empty();
    }
    void next(Callback cb, void* user) {
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(5);
        while (!output_eos) {
            if (!input_eos) {
                auto index = AMediaCodec_dequeueInputBuffer(codec, 0);
                if (index >= 0) {
                    size_t capacity = 0;
                    auto* buffer = AMediaCodec_getInputBuffer(codec, index, &capacity);
                    if (!buffer) throw std::runtime_error("Missing audio input buffer");
                    auto size = AMediaExtractor_readSampleData(extractor, buffer, capacity);
                    if (size > static_cast<ssize_t>(capacity)) throw std::runtime_error("Audio packet exceeds codec capacity");
                    input_eos = size < 0;
                    check(AMediaCodec_queueInputBuffer(codec, index, 0, input_eos ? 0 : size,
                        input_eos ? 0 : AMediaExtractor_getSampleTime(extractor), input_eos ? AMEDIACODEC_BUFFER_FLAG_END_OF_STREAM : 0));
                    if (!input_eos) AMediaExtractor_advance(extractor);
                }
            }
            AMediaCodecBufferInfo info{};
            auto index = AMediaCodec_dequeueOutputBuffer(codec, &info, 10000);
            if (index == AMEDIACODEC_INFO_OUTPUT_FORMAT_CHANGED) {
                Format value(AMediaCodec_getOutputFormat(codec), AMediaFormat_delete);
                format(value.get());
            } else if (index >= 0) {
                output_eos = info.flags & AMEDIACODEC_BUFFER_FLAG_END_OF_STREAM;
                bool emitted = false;
                try {
                    if (info.size > 0 && !(info.flags & AMEDIACODEC_BUFFER_FLAG_CODEC_CONFIG)) {
                        size_t capacity = 0;
                        auto* data = AMediaCodec_getOutputBuffer(codec, index, &capacity);
                        if (!data || info.offset < 0 || static_cast<size_t>(info.offset) + info.size > capacity) throw std::runtime_error("Invalid audio output buffer");
                        emitted = emit(data + info.offset, info.size, info.presentationTimeUs, cb, user);
                    }
                } catch (...) { AMediaCodec_releaseOutputBuffer(codec, index, false); throw; }
                check(AMediaCodec_releaseOutputBuffer(codec, index, false));
                if (emitted) return;
            }
            if (std::chrono::steady_clock::now() >= deadline) throw std::runtime_error("Android audio decoder stalled");
        }
    }
};
extern "C" {
const char* kg_error() { return last_error.c_str(); }
int kg_capabilities() {
    auto* codec = AMediaCodec_createDecoderByType("video/avc");
    if (!codec) { last_error = "No Android H.264 decoder available"; return 0; }
    AMediaCodec_delete(codec); last_error.clear(); return 3;
}
int kg_source_create(const char* path, void** result) {
    return protect([&] { auto source = std::make_unique<Source>(); source->open(path); *result = source.release(); });
}
void kg_source_destroy(void* source) { delete static_cast<Source*>(source); }
int kg_source_next(void* source, Callback callback, void* user) {
    return protect([&] { static_cast<Source*>(source)->next(callback, user); });
}
int kg_decoder_create(void** result) { return protect([&] { *result = new Decoder(); }); }
void kg_decoder_destroy(void* decoder) { delete static_cast<Decoder*>(decoder); }
int kg_decoder_push(void* decoder, const uint8_t* data, size_t len, int64_t pts, bool, Callback callback, void* user) {
    return protect([&] { static_cast<Decoder*>(decoder)->push(data, len, pts, callback, user); });
}
int kg_decoder_drain(void* decoder, Callback callback, void* user) {
    return protect([&] { static_cast<Decoder*>(decoder)->drain(callback, user); });
}
int kg_decoder_reset(void* decoder) { return protect([&] { static_cast<Decoder*>(decoder)->codec.reset(); }); }
}

extern "C" int kg_audio_create(const char* path, void** value) {
    return protect([&] { auto source = std::make_unique<AudioSource>(); source->open(path); *value = source->available ? source.release() : nullptr; });
}
extern "C" int kg_audio_next(void* value, Callback cb, void* user) { return protect([&] { static_cast<AudioSource*>(value)->next(cb, user); }); }
extern "C" void kg_audio_destroy(void* value) { delete static_cast<AudioSource*>(value); }
#include "android_encoder.h"

extern "C" int kg_decoder_create_mode(int mode, void** out) { return protect([&] { *out = new Decoder(mode); }); }
extern "C" bool kg_decoder_hardware(void*) { return false; }
