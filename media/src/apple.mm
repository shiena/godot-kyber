// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// Native Apple media services for macOS and iOS. Only Apple system frameworks are linked.
#import <AVFoundation/AVFoundation.h>
#import <VideoToolbox/VideoToolbox.h>
#include <algorithm>
#include <cstdint>
#include <cmath>
#include <cstring>
#include <stdexcept>
#include <string>
#include <vector>
#include <memory>

using Callback = void (*)(void*, const uint8_t*, size_t, uint32_t, uint32_t, int64_t);
static thread_local std::string last_error;
static void check(OSStatus status) {
    if (status != noErr) throw std::runtime_error("Apple media error: " + std::to_string(status));
}
template<class F> static int protect(F fn) noexcept {
    @autoreleasepool {
        try { fn(); last_error.clear(); return 0; }
        catch (const std::exception& e) { last_error = e.what(); return -1; }
        catch (...) { last_error = "Unknown Apple media failure"; return -1; }
    }
}
template<class T> struct Ref {
    T value = nullptr;
    ~Ref() { if (value) CFRelease(value); }
};
static void emit(CVPixelBufferRef image, CMTime time, Callback callback, void* user) {
    if (!image || CVPixelBufferGetPixelFormatType(image) != kCVPixelFormatType_32BGRA)
        throw std::runtime_error("Invalid decoded pixel buffer");
    CGRect rect = CVImageBufferGetCleanRect(image);
    if (!std::isfinite(rect.origin.x) || !std::isfinite(rect.origin.y) ||
        !std::isfinite(rect.size.width) || !std::isfinite(rect.size.height) ||
        rect.origin.x < 0 || rect.origin.y < 0 || rect.size.width <= 0 || rect.size.height <= 0 ||
        CGRectGetMaxX(rect) > CVPixelBufferGetWidth(image) || CGRectGetMaxY(rect) > CVPixelBufferGetHeight(image))
        throw std::runtime_error("Invalid video clean aperture");
    size_t x = static_cast<size_t>(rect.origin.x), y = static_cast<size_t>(rect.origin.y);
    size_t width = static_cast<size_t>(rect.size.width), height = static_cast<size_t>(rect.size.height);
    if (!width || !height || width > 8192 || height > 8192 ||
        x + width > CVPixelBufferGetWidth(image) || y + height > CVPixelBufferGetHeight(image))
        throw std::runtime_error("Invalid decoded video dimensions");
    check(CVPixelBufferLockBaseAddress(image, kCVPixelBufferLock_ReadOnly));
    try {
        const auto* base = static_cast<const uint8_t*>(CVPixelBufferGetBaseAddress(image));
        size_t stride = CVPixelBufferGetBytesPerRow(image);
        if (!base || stride < (x + width) * 4) throw std::runtime_error("Invalid video stride");
        std::vector<uint8_t> rgba(width * height * 4);
        for (size_t row = 0; row < height; ++row) {
            const auto* source = base + (y + row) * stride + x * 4;
            auto* dest = rgba.data() + row * width * 4;
            for (size_t col = 0; col < width; ++col) {
                dest[col*4] = source[col*4+2]; dest[col*4+1] = source[col*4+1];
                dest[col*4+2] = source[col*4]; dest[col*4+3] = source[col*4+3];
            }
        }
        int64_t pts = CMTIME_IS_NUMERIC(time) ? CMTimeConvertScale(time, 1000000, kCMTimeRoundingMethod_Default).value : 0;
        callback(user, rgba.data(), rgba.size(), static_cast<uint32_t>(width), static_cast<uint32_t>(height), pts);
    } catch (...) { CVPixelBufferUnlockBaseAddress(image, kCVPixelBufferLock_ReadOnly); throw; }
    CVPixelBufferUnlockBaseAddress(image, kCVPixelBufferLock_ReadOnly);
}
struct Output { CVPixelBufferRef image; CMTime pts; };
struct Decoder {
    int mode = 0;
    bool hardware = false;
    explicit Decoder(int acceleration = 0) : mode(acceleration) {}
    VTDecompressionSessionRef session = nullptr;
    CMVideoFormatDescriptionRef format = nullptr;
    std::vector<uint8_t> sps, pps;
    std::vector<Output> output;
    OSStatus failure = noErr;
    ~Decoder() { reset(); }
    void clear_session() {
        if (session) {
            VTDecompressionSessionWaitForAsynchronousFrames(session);
            VTDecompressionSessionInvalidate(session); CFRelease(session); session = nullptr;
        }
        if (format) { CFRelease(format); format = nullptr; }
    }
    void reset() {
        clear_session();
        for (auto& frame : output) if (frame.image) CVPixelBufferRelease(frame.image);
        output.clear(); sps.clear(); pps.clear(); failure = noErr;
    }
    static void decoded(void* context, void*, OSStatus status, VTDecodeInfoFlags, CVImageBufferRef image, CMTime pts, CMTime) noexcept {
        auto* self = static_cast<Decoder*>(context);
        if (status) { self->failure = status; return; }
        if (!image) return;
        CVPixelBufferRetain(image);
        try { self->output.push_back({image, pts}); }
        catch (...) { CVPixelBufferRelease(image); self->failure = kVTAllocationFailedErr; }
    }
    void deliver(Callback callback, void* user) {
        check(failure);
        // Call Rust only from the creating worker, never from VideoToolbox.
        for (auto& frame : output) {
            if (frame.image) { emit(frame.image, frame.pts, callback, user); CVPixelBufferRelease(frame.image); frame.image = nullptr; }
        }
        output.clear();
    }
    void drain(Callback callback, void* user) {
        if (session) {
            check(VTDecompressionSessionFinishDelayedFrames(session));
            check(VTDecompressionSessionWaitForAsynchronousFrames(session));
        }
        deliver(callback, user);
    }
    void create() {
        if (sps.empty() || pps.empty()) throw std::runtime_error("Missing H.264 SPS/PPS");
        const uint8_t* sets[] = {sps.data(), pps.data()};
        size_t sizes[] = {sps.size(), pps.size()};
        check(CMVideoFormatDescriptionCreateFromH264ParameterSets(kCFAllocatorDefault, 2, sets, sizes, 4, &format));
        NSDictionary* attributes = @{(id)kCVPixelBufferPixelFormatTypeKey: @(kCVPixelFormatType_32BGRA)};
        VTDecompressionOutputCallbackRecord record{decoded, this};
        NSDictionary* spec = @{
            (id)kVTVideoDecoderSpecification_EnableHardwareAcceleratedVideoDecoder: @(mode != 1),
            (id)kVTVideoDecoderSpecification_RequireHardwareAcceleratedVideoDecoder: @(mode == 2)};
        check(VTDecompressionSessionCreate(kCFAllocatorDefault, format, (__bridge CFDictionaryRef)spec,
            (__bridge CFDictionaryRef)attributes, &record, &session));
        Ref<CFTypeRef> value;
        OSStatus query = VTSessionCopyProperty(session, kVTDecompressionPropertyKey_UsingHardwareAcceleratedVideoDecoder, nullptr, &value.value);
        if (mode == 2 || (query != noErr && query != kVTPropertyNotSupportedErr)) check(query);
        hardware = value.value && CFEqual(value.value, kCFBooleanTrue);
        if ((mode == 2 && !hardware) || (mode == 1 && hardware)) throw std::runtime_error("Requested H.264 acceleration is unavailable");
    }
    void push(const uint8_t* data, size_t len, int64_t pts, Callback callback, void* user) {
        if (!len || len > 16 * 1024 * 1024 || pts < 0) throw std::runtime_error("Invalid H.264 packet");
        std::vector<std::pair<size_t,size_t>> starts;
        for (size_t i=0; i+3<=len;) {
            size_t prefix = 0;
            if (i+4<=len && !data[i] && !data[i+1] && !data[i+2] && data[i+3]==1) prefix=4;
            else if (!data[i] && !data[i+1] && data[i+2]==1) prefix=3;
            if (prefix) { starts.push_back({i,i+prefix}); i+=prefix; } else ++i;
        }
        if (starts.empty()) throw std::runtime_error("H.264 input must use Annex B");
        std::vector<uint8_t> avcc;
        bool changed = false, picture = false;
        for (size_t i=0; i<starts.size(); ++i) {
            size_t begin=starts[i].second, end=i+1<starts.size()?starts[i+1].first:len;
            while (end>begin && data[end-1]==0) --end;
            if (end<=begin) continue;
            uint8_t type=data[begin]&31;
            std::vector<uint8_t> nal(data+begin,data+end);
            if (type==7 || type==8) {
                auto& saved=type==7?sps:pps;
                if (saved!=nal) { saved=std::move(nal); changed=true; }
            } else {
                uint32_t size=static_cast<uint32_t>(nal.size());
                for (int shift=24; shift>=0; shift-=8) avcc.push_back(static_cast<uint8_t>(size>>shift));
                avcc.insert(avcc.end(),nal.begin(),nal.end());
                picture |= type==1 || type==5;
            }
        }
        if (changed && session) { drain(callback,user); clear_session(); }
        if (!picture) return;
        if (!session) create();
        Ref<CMBlockBufferRef> block;
        check(CMBlockBufferCreateWithMemoryBlock(kCFAllocatorDefault,nullptr,avcc.size(),kCFAllocatorDefault,nullptr,0,avcc.size(),0,&block.value));
        check(CMBlockBufferReplaceDataBytes(avcc.data(),block.value,0,avcc.size()));
        CMSampleTimingInfo timing{kCMTimeInvalid,CMTimeMake(pts,1000000),kCMTimeInvalid};
        size_t size=avcc.size(); Ref<CMSampleBufferRef> sample;
        check(CMSampleBufferCreateReady(kCFAllocatorDefault,block.value,format,1,1,&timing,1,&size,&sample.value));
        check(VTDecompressionSessionDecodeFrame(session,sample.value,0,nullptr,nullptr));
        check(VTDecompressionSessionWaitForAsynchronousFrames(session));
        deliver(callback,user);
    }
};
struct Source {
    AVAssetReader* reader;
    AVAssetReaderTrackOutput* output;
    explicit Source(const char* path) {
        NSString* name=[[NSString alloc] initWithUTF8String:path];
        if (!name || ![[NSFileManager defaultManager] fileExistsAtPath:name]) throw std::runtime_error("Video file not found");
        AVURLAsset* asset=[AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:name] options:nil];
        AVAssetTrack* track=[[asset tracksWithMediaType:AVMediaTypeVideo] firstObject];
        if (!track) throw std::runtime_error("No readable video track");
        NSError* error=nil;
        reader=[[AVAssetReader alloc] initWithAsset:asset error:&error];
        if (!reader) throw std::runtime_error(error.localizedDescription.UTF8String ?: "Cannot open video");
        output=[[AVAssetReaderTrackOutput alloc] initWithTrack:track outputSettings:@{(id)kCVPixelBufferPixelFormatTypeKey:@(kCVPixelFormatType_32BGRA)}];
        output.alwaysCopiesSampleData=NO;
        if (![reader canAddOutput:output]) throw std::runtime_error("Unsupported video output");
        [reader addOutput:output];
        if (![reader startReading]) throw std::runtime_error(reader.error.localizedDescription.UTF8String ?: "Cannot start video reader");
    }
    ~Source() { [reader cancelReading]; }
    void next(Callback callback,void* user) {
        Ref<CMSampleBufferRef> sample; sample.value=[output copyNextSampleBuffer];
        if (!sample.value) {
            if (reader.status==AVAssetReaderStatusFailed) throw std::runtime_error(reader.error.localizedDescription.UTF8String ?: "Video read failed");
            return;
        }
        emit(CMSampleBufferGetImageBuffer(sample.value),CMSampleBufferGetPresentationTimeStamp(sample.value),callback,user);
    }
};
// Keep container timestamps so independently decoded audio and video share a timeline.
struct AudioSource {
    AVAssetReader* reader = nil;
    AVAssetReaderTrackOutput* output = nil;
    bool available = false;
    explicit AudioSource(const char* path) {
        NSString* name = [[NSString alloc] initWithUTF8String:path];
        if (!name || ![[NSFileManager defaultManager] fileExistsAtPath:name]) throw std::runtime_error("Audio file not found");
        AVURLAsset* asset = [AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:name] options:nil];
        AVAssetTrack* track = [[asset tracksWithMediaType:AVMediaTypeAudio] firstObject];
        if (!track) return;
        NSError* error = nil;
        reader = [[AVAssetReader alloc] initWithAsset:asset error:&error];
        if (!reader) throw std::runtime_error(error.localizedDescription.UTF8String ?: "Cannot open audio");
        NSDictionary* settings = @{
            AVFormatIDKey: @(kAudioFormatLinearPCM), AVSampleRateKey: @48000,
            AVNumberOfChannelsKey: @2, AVLinearPCMBitDepthKey: @32,
            AVLinearPCMIsFloatKey: @YES, AVLinearPCMIsBigEndianKey: @NO,
            AVLinearPCMIsNonInterleaved: @NO
        };
        output = [[AVAssetReaderTrackOutput alloc] initWithTrack:track outputSettings:settings];
        output.alwaysCopiesSampleData = NO;
        if (![reader canAddOutput:output]) throw std::runtime_error("Unsupported audio output");
        [reader addOutput:output];
        if (![reader startReading]) throw std::runtime_error(reader.error.localizedDescription.UTF8String ?: "Cannot start audio reader");
        available = true;
    }
    ~AudioSource() { [reader cancelReading]; }
    void next(Callback callback, void* user) {
        Ref<CMSampleBufferRef> sample; sample.value = [output copyNextSampleBuffer];
        if (!sample.value) {
            if (reader.status == AVAssetReaderStatusFailed) throw std::runtime_error(reader.error.localizedDescription.UTF8String ?: "Audio read failed");
            return;
        }
        const auto* format = CMAudioFormatDescriptionGetStreamBasicDescription(CMSampleBufferGetFormatDescription(sample.value));
        if (!format || format->mFormatID != kAudioFormatLinearPCM || format->mSampleRate != 48000 ||
            format->mChannelsPerFrame != 2 || format->mBitsPerChannel != 32 || format->mBytesPerFrame != 8 ||
            !(format->mFormatFlags & kAudioFormatFlagIsFloat) || (format->mFormatFlags & (kAudioFormatFlagIsNonInterleaved | kAudioFormatFlagIsBigEndian)))
            throw std::runtime_error("Unsupported decoded audio format");
        CMBlockBufferRef block = CMSampleBufferGetDataBuffer(sample.value);
        if (!block) throw std::runtime_error("Missing decoded audio buffer");
        size_t size = CMBlockBufferGetDataLength(block);
        if (size % 8 || size > 48000 * 8 * 10) throw std::runtime_error("Invalid decoded audio size");
        std::vector<uint8_t> pcm(size);
        check(CMBlockBufferCopyDataBytes(block, 0, size, pcm.data()));
        CMTime time = CMSampleBufferGetPresentationTimeStamp(sample.value);
        if (!CMTIME_IS_NUMERIC(time)) throw std::runtime_error("Invalid audio timestamp");
        int64_t pts = CMTimeConvertScale(time, 1000000, kCMTimeRoundingMethod_Default).value;
        callback(user, pcm.data(), pcm.size(), 48000, 2, pts);
    }
};
extern "C" {
const char* kg_error() { return last_error.c_str(); }
int kg_capabilities() { return 3; }
int kg_decoder_create(void** value) { return protect([&]{*value=new Decoder();}); }
void kg_decoder_destroy(void* value) { delete static_cast<Decoder*>(value); }
int kg_decoder_push(void* value,const uint8_t* data,size_t len,int64_t pts,bool,Callback cb,void* user) { return protect([&]{static_cast<Decoder*>(value)->push(data,len,pts,cb,user);}); }
int kg_decoder_drain(void* value,Callback cb,void* user) { return protect([&]{static_cast<Decoder*>(value)->drain(cb,user);}); }
int kg_decoder_reset(void* value) { return protect([&]{static_cast<Decoder*>(value)->reset();}); }
int kg_source_create(const char* path,void** value) { return protect([&]{*value=new Source(path);}); }
int kg_source_next(void* value,Callback cb,void* user) { return protect([&]{static_cast<Source*>(value)->next(cb,user);}); }
void kg_source_destroy(void* value) { @autoreleasepool { delete static_cast<Source*>(value); } }
}

extern "C" int kg_audio_create(const char* path, void** value) {
    return protect([&] {
        auto source = std::make_unique<AudioSource>(path);
        *value = source->available ? source.release() : nullptr;
    });
}
extern "C" int kg_audio_next(void* value, Callback cb, void* user) {
    return protect([&] { static_cast<AudioSource*>(value)->next(cb, user); });
}
extern "C" void kg_audio_destroy(void* value) { @autoreleasepool { delete static_cast<AudioSource*>(value); } }
#include "apple_encoder.h"

extern "C" int kg_decoder_create_mode(int mode, void** out) { return protect([&] { *out = new Decoder(mode); }); }
extern "C" bool kg_decoder_hardware(void* value) { return static_cast<Decoder*>(value)->hardware; }
