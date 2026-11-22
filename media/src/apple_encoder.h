// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// VideoToolbox encoder. Output callbacks retain samples; only the worker calls Rust.
using PacketCallback = void (*)(void*, const uint8_t*, size_t, int64_t, bool);
struct Encoder {
    VTCompressionSessionRef session = nullptr;
    CMSampleBufferRef output = nullptr;
    OSStatus failure = noErr;
    uint32_t width, height, fps;
    bool hardware = false;
    Encoder(uint32_t w, uint32_t h, uint32_t rate, uint32_t bitrate, int mode) : width(w), height(h), fps(rate) {
        try {
            NSDictionary* spec = @{
                (id)kVTVideoEncoderSpecification_EnableHardwareAcceleratedVideoEncoder: @(mode != 1),
                (id)kVTVideoEncoderSpecification_RequireHardwareAcceleratedVideoEncoder: @(mode == 2)};
            NSDictionary* attrs = @{(id)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_32BGRA),
                (id)kCVPixelBufferWidthKey: @(w), (id)kCVPixelBufferHeightKey: @(h),
                (id)kCVPixelBufferIOSurfacePropertiesKey: @{}};
            check(VTCompressionSessionCreate(kCFAllocatorDefault, w, h, kCMVideoCodecType_H264,
                (__bridge CFDictionaryRef)spec, (__bridge CFDictionaryRef)attrs, nullptr, encoded, this, &session));
            NSDictionary* props = @{
                (id)kVTCompressionPropertyKey_RealTime: @YES,
                (id)kVTCompressionPropertyKey_AllowFrameReordering: @NO,
                (id)kVTCompressionPropertyKey_MaxKeyFrameInterval: @1,
                (id)kVTCompressionPropertyKey_ExpectedFrameRate: @(rate),
                (id)kVTCompressionPropertyKey_AverageBitRate: @(bitrate),
                (id)kVTCompressionPropertyKey_YCbCrMatrix: (id)kCVImageBufferYCbCrMatrix_ITU_R_601_4,
                (id)kVTCompressionPropertyKey_ProfileLevel: (id)kVTProfileLevel_H264_Baseline_AutoLevel};
            for (id key in props) {
                OSStatus status = VTSessionSetProperty(session, (__bridge CFStringRef)key, (__bridge CFTypeRef)props[key]);
                if (status) throw std::runtime_error("VideoToolbox property " + std::string([key UTF8String]) + ": " + std::to_string(status));
            }
            check(VTCompressionSessionPrepareToEncodeFrames(session));
            Ref<CFTypeRef> value;
            OSStatus query = VTSessionCopyProperty(session, kVTCompressionPropertyKey_UsingHardwareAcceleratedVideoEncoder, nullptr, &value.value);
            if (mode == 2 || (query != noErr && query != kVTPropertyNotSupportedErr)) check(query);
            hardware = value.value && CFEqual(value.value, kCFBooleanTrue);
            if ((mode == 2 && !hardware) || (mode == 1 && hardware)) throw std::runtime_error("Requested H.264 acceleration is unavailable");
        } catch (...) { clear(); throw; }
    }
    void clear() {
        if (session) { VTCompressionSessionInvalidate(session); CFRelease(session); session = nullptr; }
        if (output) { CFRelease(output); output = nullptr; }
    }
    ~Encoder() { clear(); }
    static void encoded(void* user, void*, OSStatus status, VTEncodeInfoFlags, CMSampleBufferRef sample) noexcept {
        auto* self = static_cast<Encoder*>(user);
        if (status) { self->failure = status; return; }
        if (self->output || !sample) { self->failure = kVTVideoEncoderMalfunctionErr; return; }
        CFRetain(sample);
        self->output = sample;
    }
    void deliver(PacketCallback cb, void* user) {
        check(failure);
        if (!output) return;
        std::vector<uint8_t> packet;
        auto append = [&](const uint8_t* p, size_t n) {
            packet.insert(packet.end(), {0, 0, 0, 1}); packet.insert(packet.end(), p, p + n);
        };
        auto format = CMSampleBufferGetFormatDescription(output);
        size_t count = 0; int nal_size = 0;
        check(CMVideoFormatDescriptionGetH264ParameterSetAtIndex(format, 0, nullptr, nullptr, &count, &nal_size));
        for (size_t i = 0; i < count; ++i) {
            const uint8_t* p; size_t n;
            check(CMVideoFormatDescriptionGetH264ParameterSetAtIndex(format, i, &p, &n, nullptr, nullptr));
            append(p, n);
        }
        if (nal_size != 4) throw std::runtime_error("Unsupported H.264 length prefix");
        auto block = CMSampleBufferGetDataBuffer(output);
        std::vector<uint8_t> bytes(CMBlockBufferGetDataLength(block));
        check(CMBlockBufferCopyDataBytes(block, 0, bytes.size(), bytes.data()));
        for (size_t i = 0; i < bytes.size();) {
            if (bytes.size() - i < 4) throw std::runtime_error("Truncated H.264 NAL length");
            uint32_t n = (uint32_t(bytes[i]) << 24) | (uint32_t(bytes[i+1]) << 16) | (uint32_t(bytes[i+2]) << 8) | bytes[i+3]; i += 4;
            if (n > bytes.size() - i) throw std::runtime_error("Invalid H.264 NAL length");
            append(bytes.data() + i, n); i += n;
        }
        bool key = true;
        CFArrayRef attachments = CMSampleBufferGetSampleAttachmentsArray(output, false);
        if (attachments && CFArrayGetCount(attachments)) {
            auto dict = static_cast<CFDictionaryRef>(CFArrayGetValueAtIndex(attachments, 0));
            key = !CFDictionaryContainsKey(dict, kCMSampleAttachmentKey_NotSync);
        }
        auto time = CMTimeConvertScale(CMSampleBufferGetPresentationTimeStamp(output), 1000000, kCMTimeRoundingMethod_Default);
        cb(user, packet.data(), packet.size(), time.value, key);
        CFRelease(output); output = nullptr;
    }
    void push(const uint8_t* rgba, size_t len, int64_t pts, PacketCallback cb, void* user) {
        if (len != size_t(width) * height * 4 || pts < 0) throw std::runtime_error("Invalid encoder frame");
        Ref<CVPixelBufferRef> image;
        check(CVPixelBufferPoolCreatePixelBuffer(nullptr, VTCompressionSessionGetPixelBufferPool(session), &image.value));
        CVBufferSetAttachment(image.value, kCVImageBufferYCbCrMatrixKey, kCVImageBufferYCbCrMatrix_ITU_R_601_4, kCVAttachmentMode_ShouldPropagate);
        check(CVPixelBufferLockBaseAddress(image.value, 0));
        auto* base = static_cast<uint8_t*>(CVPixelBufferGetBaseAddress(image.value));
        auto stride = CVPixelBufferGetBytesPerRow(image.value);
        for (uint32_t y = 0; y < height; ++y) for (uint32_t x = 0; x < width; ++x) {
            auto p = rgba + (size_t(y) * width + x) * 4; auto d = base + y * stride + x * 4;
            d[0] = p[2]; d[1] = p[1]; d[2] = p[0]; d[3] = 255;
        }
        check(CVPixelBufferUnlockBaseAddress(image.value, 0));
        NSDictionary* options = @{(id)kVTEncodeFrameOptionKey_ForceKeyFrame: @YES};
        check(VTCompressionSessionEncodeFrame(session, image.value, CMTimeMake(pts, 1000000), CMTimeMake(1, fps),
            (__bridge CFDictionaryRef)options, nullptr, nullptr));
        check(VTCompressionSessionCompleteFrames(session, kCMTimeInvalid));
        deliver(cb, user);
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
    return protect([&] { auto* encoder = static_cast<Encoder*>(value); check(VTCompressionSessionCompleteFrames(encoder->session, kCMTimeInvalid)); encoder->deliver(cb, user); });
}
}
