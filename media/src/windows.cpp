// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// Windows Media Foundation implementation. No redistributable codec libraries.
#define NOMINMAX
#include <windows.h>
#include <mfapi.h>
#include <mfidl.h>
#include <mfreadwrite.h>
#include <mferror.h>
#include <wmcodecdsp.h>
#include <wrl/client.h>
#include <d3d11.h>
#include <d3d10.h>
#include <codecapi.h>
#include <algorithm>
#include <cstdint>
#include <memory>
#include <vector>

using Microsoft::WRL::ComPtr;
using Callback = void (*)(void*, const uint8_t*, size_t, uint32_t, uint32_t, int64_t);
struct Failure { HRESULT hr; };
static void check(HRESULT hr) { if (FAILED(hr)) throw Failure{hr}; }

// Resolve optional OS media components without making the whole extension depend
// on their presence (for example, Windows N without the Media Feature Pack).
struct SystemLibrary {
    HMODULE module;
    explicit SystemLibrary(const wchar_t* name) : module(LoadLibraryExW(name, nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32)) {
        if (!module) throw Failure{HRESULT_FROM_WIN32(GetLastError())};
    }
    ~SystemLibrary() { FreeLibrary(module); }
    template<class T> T get(const char* name) {
        auto address = GetProcAddress(module, name);
        if (!address) throw Failure{HRESULT_FROM_WIN32(GetLastError())};
        return reinterpret_cast<T>(address);
    }
};
struct MediaApi {
    SystemLibrary platform{L"mfplat.dll"}, reader{L"mfreadwrite.dll"};
    decltype(&MFStartup) startup = platform.get<decltype(&MFStartup)>("MFStartup");
    decltype(&MFShutdown) shutdown = platform.get<decltype(&MFShutdown)>("MFShutdown");
    decltype(&MFCreateMediaType) media_type = platform.get<decltype(&MFCreateMediaType)>("MFCreateMediaType");
    decltype(&MFCreateSample) sample = platform.get<decltype(&MFCreateSample)>("MFCreateSample");
    decltype(&MFCreateAttributes) attributes = platform.get<decltype(&MFCreateAttributes)>("MFCreateAttributes");
    decltype(&MFCreateAlignedMemoryBuffer) buffer = platform.get<decltype(&MFCreateAlignedMemoryBuffer)>("MFCreateAlignedMemoryBuffer");
    decltype(&MFCreateSourceReaderFromURL) source = reader.get<decltype(&MFCreateSourceReaderFromURL)>("MFCreateSourceReaderFromURL");
    decltype(&MFCreateDXGIDeviceManager) manager = platform.get<decltype(&MFCreateDXGIDeviceManager)>("MFCreateDXGIDeviceManager");
    decltype(&MFTEnumEx) enumerate = platform.get<decltype(&MFTEnumEx)>("MFTEnumEx");
};
static MediaApi& media_api() { static MediaApi api; return api; }
#define MFStartup media_api().startup
#define MFShutdown media_api().shutdown
#define MFCreateMediaType media_api().media_type
#define MFCreateSample media_api().sample
#define MFCreateAttributes media_api().attributes
#define MFCreateAlignedMemoryBuffer media_api().buffer
#define MFCreateSourceReaderFromURL media_api().source

struct Runtime {
    bool com = false, mf = false;
    Runtime() {
        // Resolve libraries before initializing COM so a load failure cannot leak
        // a COM initialization count during constructor unwinding.
        media_api();
        HRESULT hr = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
        if (hr != RPC_E_CHANGED_MODE) { check(hr); com = true; }
        hr = MFStartup(MF_VERSION, MFSTARTUP_FULL);
        if (FAILED(hr)) { if (com) CoUninitialize(); throw Failure{hr}; }
        mf = true;
    }
    ~Runtime() { if (mf) MFShutdown(); if (com) CoUninitialize(); }
};
template<class F> static HRESULT protect(F f) noexcept {
    try { f(); return S_OK; }
    catch (Failure e) { return e.hr; }
    catch (...) { return E_FAIL; }
}
static ComPtr<IMFMediaType> video_type(GUID format) {
    ComPtr<IMFMediaType> type;
    check(MFCreateMediaType(&type));
    check(type->SetGUID(MF_MT_MAJOR_TYPE, MFMediaType_Video));
    check(type->SetGUID(MF_MT_SUBTYPE, format));
    return type;
}
static ComPtr<IMFSample> sample_buffer(DWORD size, DWORD alignment = 0) {
    ComPtr<IMFSample> sample;
    ComPtr<IMFMediaBuffer> buffer;
    check(MFCreateSample(&sample));
    check(MFCreateAlignedMemoryBuffer(size, alignment, &buffer));
    check(sample->AddBuffer(buffer.Get()));
    return sample;
}
static void emit_rgb(IMFSample* sample, IMFMediaType* type, Callback cb, void* user) {
    UINT32 width, height;
    check(MFGetAttributeSize(type, MF_MT_FRAME_SIZE, &width, &height));
    if (!width || !height || width > 8192 || height > 8192) throw Failure{MF_E_INVALIDMEDIATYPE};
    const UINT32 coded_width = width, coded_height = height;
    UINT32 left = 0, top = 0;
    MFVideoArea area{};
    if (SUCCEEDED(type->GetBlob(MF_MT_MINIMUM_DISPLAY_APERTURE, reinterpret_cast<BYTE*>(&area), sizeof(area), nullptr)) ||
        SUCCEEDED(type->GetBlob(MF_MT_GEOMETRIC_APERTURE, reinterpret_cast<BYTE*>(&area), sizeof(area), nullptr))) {
        if (area.OffsetX.value < 0 || area.OffsetY.value < 0 || area.Area.cx <= 0 || area.Area.cy <= 0)
            throw Failure{MF_E_INVALIDMEDIATYPE};
        left = area.OffsetX.value; top = area.OffsetY.value;
        width = area.Area.cx; height = area.Area.cy;
        if (static_cast<uint64_t>(left) + width > coded_width || static_cast<uint64_t>(top) + height > coded_height)
            throw Failure{MF_E_INVALIDMEDIATYPE};
    }
    ComPtr<IMFMediaBuffer> buffer;
    check(sample->ConvertToContiguousBuffer(&buffer));
    ComPtr<IMF2DBuffer> buffer2d;
    BYTE* pixels = nullptr;
    LONG stride = static_cast<LONG>(MFGetAttributeUINT32(type, MF_MT_DEFAULT_STRIDE, coded_width * 4));
    DWORD length = 0;
    bool locked2d = SUCCEEDED(buffer.As(&buffer2d));
    if (locked2d) { check(buffer2d->Lock2D(&pixels, &stride)); }
    else {
        check(buffer->Lock(&pixels, nullptr, &length));
        if (static_cast<uint64_t>(std::abs(static_cast<int64_t>(stride))) * coded_height > length) { buffer->Unlock(); throw Failure{E_INVALIDARG}; }
        if (stride < 0) pixels += static_cast<size_t>(-static_cast<int64_t>(stride)) * (coded_height - 1);
    }
    // Always unlock even if allocation fails.
    try {
        if (std::abs(static_cast<int64_t>(stride)) < static_cast<int64_t>(coded_width) * 4) throw Failure{E_INVALIDARG};
        std::vector<uint8_t> rgba(static_cast<size_t>(width) * height * 4);
        for (UINT32 y = 0; y < height; ++y) {
            const BYTE* row = pixels + static_cast<ptrdiff_t>(y + top) * stride + left * 4;
            for (UINT32 x = 0; x < width; ++x) {
                size_t out = (static_cast<size_t>(y) * width + x) * 4;
                rgba[out] = row[x * 4 + 2]; rgba[out + 1] = row[x * 4 + 1];
                rgba[out + 2] = row[x * 4]; rgba[out + 3] = 255;
            }
        }
        LONGLONG pts = 0;
        sample->GetSampleTime(&pts);
        cb(user, rgba.data(), rgba.size(), width, height, pts / 10);
    } catch (...) {
        if (locked2d) buffer2d->Unlock2D(); else buffer->Unlock();
        throw;
    }
    if (locked2d) buffer2d->Unlock2D(); else buffer->Unlock();
}
// Ownership of MFT-created samples and event collections stays in this helper.
static HRESULT output(IMFTransform* transform, ComPtr<IMFSample>& sample) {
    MFT_OUTPUT_STREAM_INFO info{};
    check(transform->GetOutputStreamInfo(0, &info));
    if (!(info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES))
        sample = sample_buffer(info.cbSize, info.cbAlignment);
    MFT_OUTPUT_DATA_BUFFER out{};
    out.pSample = sample.Get();
    DWORD status = 0;
    HRESULT hr = transform->ProcessOutput(0, 1, &out, &status);
    if (out.pEvents) out.pEvents->Release();
    if (out.pSample && out.pSample != sample.Get()) sample.Attach(out.pSample);
    return hr;
}
#include "windows_gpu.h"
struct Decoder {
    Runtime runtime;
    ComPtr<ID3D11Device> device;
    ComPtr<ID3D11DeviceContext> context;
    ComPtr<IMFDXGIDeviceManager> manager;
    ComPtr<IMFTransform> decoder, converter;
    ComPtr<IMFMediaType> rgb_type;
    bool hardware = false;
    GpuContext* gpu = nullptr;
    GpuCallback gpu_callback = nullptr;
    explicit Decoder(int acceleration = 1, GpuContext* sharing = nullptr) : gpu(sharing) {
        check(CoCreateInstance(CLSID_CMSH264DecoderMFT, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&decoder)));
        if (acceleration != 1) {
            HRESULT hr = protect([&] {
                static SystemLibrary d3d{L"d3d11.dll"};
                auto create = d3d.get<decltype(&D3D11CreateDevice)>("D3D11CreateDevice");
                if (gpu) { device = gpu->device; context = gpu->context; }
                else check(create(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr,
                    D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    nullptr, 0, D3D11_SDK_VERSION, &device, nullptr, &context));
                ComPtr<ID3D10Multithread> multithread;
                check(device.As(&multithread));
                multithread->SetMultithreadProtected(TRUE);
                UINT token;
                check(media_api().manager(&token, &manager));
                check(manager->ResetDevice(device.Get(), token));
                check(decoder->ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER,
                    reinterpret_cast<ULONG_PTR>(manager.Get())));
            });
            if (FAILED(hr)) {
                if (acceleration == 2) check(hr);
                manager.Reset(); context.Reset(); device.Reset();
            }
        }
        auto input = video_type(MFVideoFormat_H264);
        check(decoder->SetInputType(0, input.Get(), 0));
        configure();
        ComPtr<IMFAttributes> attributes;
        if (SUCCEEDED(decoder->GetAttributes(&attributes))) attributes->SetUINT32(MF_LOW_LATENCY, TRUE);
        check(decoder->ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0));
        check(decoder->ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0));
    }
    void configure() {
        ComPtr<IMFMediaType> type;
        for (DWORD i = 0;; ++i) {
            check(decoder->GetOutputAvailableType(0, i, &type));
            GUID subtype;
            check(type->GetGUID(MF_MT_SUBTYPE, &subtype));
            if (subtype == MFVideoFormat_NV12) break;
            type.Reset();
        }
        check(decoder->SetOutputType(0, type.Get(), 0));
        converter.Reset();
        check(CoCreateInstance(CLSID_CColorConvertDMO, nullptr, CLSCTX_INPROC_SERVER, IID_PPV_ARGS(&converter)));
        check(converter->SetInputType(0, type.Get(), 0));
        check(MFCreateMediaType(&rgb_type));
        check(type->CopyAllItems(rgb_type.Get()));
        check(rgb_type->SetGUID(MF_MT_SUBTYPE, MFVideoFormat_RGB32));
        UINT32 w, h;
        check(MFGetAttributeSize(type.Get(), MF_MT_FRAME_SIZE, &w, &h));
        if (!w || !h || w > 8192 || h > 8192) throw Failure{MF_E_INVALIDMEDIATYPE};
        check(rgb_type->SetUINT32(MF_MT_DEFAULT_STRIDE, w * 4));
        check(rgb_type->SetUINT32(MF_MT_SAMPLE_SIZE, w * h * 4));
        check(converter->SetOutputType(0, rgb_type.Get(), 0));
        check(converter->ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0));
        check(converter->ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0));
    }
    void receive(Callback cb, void* user) {
        for (;;) {
            ComPtr<IMFSample> decoded;
            HRESULT hr = output(decoder.Get(), decoded);
            if (hr == MF_E_TRANSFORM_NEED_MORE_INPUT) return;
            if (hr == MF_E_TRANSFORM_STREAM_CHANGE) { configure(); continue; }
            check(hr);
            if (!decoded || !converter) throw Failure{E_UNEXPECTED};
            ComPtr<IMFMediaBuffer> gpu_buffer;
            check(decoded->GetBufferByIndex(0, &gpu_buffer));
            ComPtr<IMFDXGIBuffer> dxgi;
            if (SUCCEEDED(gpu_buffer.As(&dxgi))) {
                ComPtr<ID3D11Texture2D> texture;
                check(dxgi->GetResource(IID_PPV_ARGS(&texture)));
                UINT index;
                check(dxgi->GetSubresourceIndex(&index));
                if (gpu && gpu_callback) {
                    auto shared = convert_gpu(*gpu, texture.Get(), index, rgb_type.Get());
                    LONGLONG pts = 0; decoded->GetSampleTime(&pts);
                    auto* frame = shared.release();
                    gpu_callback(user, frame, frame->width, frame->height, pts / 10);
                    hardware = true;
                    continue;
                }
                D3D11_TEXTURE2D_DESC desc;
                texture->GetDesc(&desc);
                if (desc.Format != DXGI_FORMAT_NV12) throw Failure{MF_E_INVALIDMEDIATYPE};
                desc.ArraySize = 1; desc.MipLevels = 1;
                desc.Usage = D3D11_USAGE_STAGING; desc.BindFlags = 0;
                desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ; desc.MiscFlags = 0;
                ComPtr<ID3D11Texture2D> staging;
                check(device->CreateTexture2D(&desc, nullptr, &staging));
                context->CopySubresourceRegion(staging.Get(), 0, 0, 0, 0, texture.Get(), index, nullptr);
                auto cpu = sample_buffer(desc.Width * desc.Height * 3 / 2);
                ComPtr<IMFMediaBuffer> bytes;
                check(cpu->GetBufferByIndex(0, &bytes));
                BYTE* target;
                check(bytes->Lock(&target, nullptr, nullptr));
                D3D11_MAPPED_SUBRESOURCE mapped{};
                HRESULT mapped_hr = context->Map(staging.Get(), 0, D3D11_MAP_READ, 0, &mapped);
                if (FAILED(mapped_hr)) { bytes->Unlock(); check(mapped_hr); }
                for (UINT y = 0; y < desc.Height * 3 / 2; ++y)
                    memcpy(target + y * desc.Width, static_cast<BYTE*>(mapped.pData) + y * mapped.RowPitch, desc.Width);
                context->Unmap(staging.Get(), 0);
                check(bytes->Unlock());
                check(bytes->SetCurrentLength(desc.Width * desc.Height * 3 / 2));
                LONGLONG pts = 0;
                decoded->GetSampleTime(&pts);
                check(cpu->SetSampleTime(pts));
                decoded = cpu;
                hardware = true;
            }
            check(converter->ProcessInput(0, decoded.Get(), 0));
            ComPtr<IMFSample> rgb;
            check(output(converter.Get(), rgb));
            LONGLONG pts = 0;
            decoded->GetSampleTime(&pts);
            check(rgb->SetSampleTime(pts));
            emit_rgb(rgb.Get(), rgb_type.Get(), cb, user);
        }
    }
    void push(const uint8_t* data, size_t len, int64_t pts, bool key, Callback cb, void* user) {
        if (!len || len > 16 * 1024 * 1024 || pts > INT64_MAX / 10 || pts < INT64_MIN / 10) throw Failure{E_INVALIDARG};
        auto sample = sample_buffer(static_cast<DWORD>(len));
        ComPtr<IMFMediaBuffer> buffer;
        check(sample->GetBufferByIndex(0, &buffer));
        BYTE* target;
        check(buffer->Lock(&target, nullptr, nullptr));
        memcpy(target, data, len);
        check(buffer->Unlock());
        check(buffer->SetCurrentLength(static_cast<DWORD>(len)));
        check(sample->SetSampleTime(pts * 10));
        check(sample->SetSampleDuration(0));
        check(sample->SetUINT32(MFSampleExtension_CleanPoint, key));
        HRESULT hr = decoder->ProcessInput(0, sample.Get(), 0);
        if (hr == MF_E_NOTACCEPTING) { receive(cb, user); hr = decoder->ProcessInput(0, sample.Get(), 0); }
        check(hr);
        receive(cb, user);
    }
};
struct Source {
    Runtime runtime;
    ComPtr<IMFSourceReader> reader;
    explicit Source(const wchar_t* path) {
        ComPtr<IMFAttributes> attributes;
        check(MFCreateAttributes(&attributes, 2));
        check(attributes->SetUINT32(MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, TRUE));
        check(MFCreateSourceReaderFromURL(path, attributes.Get(), &reader));
        check(reader->SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS, FALSE));
        check(reader->SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM, TRUE));
        auto type = video_type(MFVideoFormat_RGB32);
        check(reader->SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM, nullptr, type.Get()));
    }
    void next(Callback cb, void* user) {
        for (;;) {
            DWORD flags = 0;
            LONGLONG pts = 0;
            ComPtr<IMFSample> sample;
            check(reader->ReadSample(MF_SOURCE_READER_FIRST_VIDEO_STREAM, 0, nullptr, &flags, &pts, &sample));
            if (flags & MF_SOURCE_READERF_ERROR) throw Failure{E_FAIL};
            if (sample) {
                ComPtr<IMFMediaType> type;
                check(reader->GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM, &type));
                check(sample->SetSampleTime(pts));
                emit_rgb(sample.Get(), type.Get(), cb, user);
                return;
            }
            if (flags & MF_SOURCE_READERF_ENDOFSTREAM) return;
        }
    }
};
// Audio and video readers retain the same container presentation timestamps.
struct AudioSource {
    Runtime runtime;
    ComPtr<IMFSourceReader> reader;
    bool available = false;
    explicit AudioSource(const wchar_t* path) {
        check(MFCreateSourceReaderFromURL(path, nullptr, &reader));
        check(reader->SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS, FALSE));
        ComPtr<IMFMediaType> native;
        HRESULT hr = reader->GetNativeMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM, 0, &native);
        if (hr == MF_E_INVALIDSTREAMNUMBER) return;
        check(hr);
        check(reader->SetStreamSelection(MF_SOURCE_READER_FIRST_AUDIO_STREAM, TRUE));
        ComPtr<IMFMediaType> type;
        check(MFCreateMediaType(&type));
        check(type->SetGUID(MF_MT_MAJOR_TYPE, MFMediaType_Audio));
        check(type->SetGUID(MF_MT_SUBTYPE, MFAudioFormat_Float));
        check(type->SetUINT32(MF_MT_AUDIO_NUM_CHANNELS, 2));
        check(type->SetUINT32(MF_MT_AUDIO_SAMPLES_PER_SECOND, 48000));
        check(type->SetUINT32(MF_MT_AUDIO_BITS_PER_SAMPLE, 32));
        check(type->SetUINT32(MF_MT_AUDIO_BLOCK_ALIGNMENT, 8));
        check(type->SetUINT32(MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 48000 * 8));
        check(reader->SetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM, nullptr, type.Get()));
        available = true;
    }
    void next(Callback cb, void* user) {
        for (;;) {
            DWORD flags = 0;
            LONGLONG pts = 0;
            ComPtr<IMFSample> sample;
            check(reader->ReadSample(MF_SOURCE_READER_FIRST_AUDIO_STREAM, 0, nullptr, &flags, &pts, &sample));
            if (flags & MF_SOURCE_READERF_ERROR) throw Failure{E_FAIL};
            if (sample) {
                ComPtr<IMFMediaBuffer> buffer;
                check(sample->ConvertToContiguousBuffer(&buffer));
                BYTE* bytes = nullptr;
                DWORD size = 0;
                check(buffer->Lock(&bytes, nullptr, &size));
                cb(user, bytes, size, 48000, 2, pts / 10);
                check(buffer->Unlock());
                return;
            }
            if (flags & MF_SOURCE_READERF_ENDOFSTREAM) return;
        }
    }
};
extern "C" {
HRESULT km_available() { return protect([] { Runtime runtime; }); }
HRESULT km_decoder_create(void** result) { return protect([&] { *result = new Decoder(); }); }
HRESULT km_decoder_create_mode(int mode, void** result) { return protect([&] { *result = new Decoder(mode); }); }
HRESULT km_gpu_context_create(void* device, void** result) { return protect([&] { *result = new GpuContext(static_cast<ID3D12Device*>(device)); }); }
void km_gpu_context_destroy(void* value) { delete static_cast<GpuContext*>(value); }
HRESULT km_gpu_decoder_create(void* context, void** result) { return protect([&] { *result = new Decoder(2, static_cast<GpuContext*>(context)); }); }
void km_gpu_frame_destroy(void* value) { delete static_cast<GpuFrame*>(value); }
void* km_gpu_frame_resource(void* value) { return static_cast<GpuFrame*>(value)->resource.Get(); }
HRESULT km_gpu_decoder_push(void* value, const uint8_t* data, size_t len, int64_t pts, GpuCallback cb, void* user) {
    return protect([&] {
        auto& d = *static_cast<Decoder*>(value);
        d.gpu_callback = cb;
        d.push(data, len, pts, true, [](void*, const uint8_t*, size_t, uint32_t, uint32_t, int64_t) { throw Failure{MF_E_UNSUPPORTED_D3D_TYPE}; }, user);
    });
}
bool km_decoder_hardware(void* value) { return static_cast<Decoder*>(value)->hardware; }
void km_decoder_destroy(void* value) { delete static_cast<Decoder*>(value); }
HRESULT km_decoder_push(void* value, const uint8_t* data, size_t len, int64_t pts, bool key, Callback cb, void* user) {
    return protect([&] { static_cast<Decoder*>(value)->push(data, len, pts, key, cb, user); });
}
HRESULT km_decoder_drain(void* value, Callback cb, void* user) {
    return protect([&] {
        auto& d = *static_cast<Decoder*>(value);
        check(d.decoder->ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0));
        check(d.decoder->ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0));
        d.receive(cb, user);
    });
}
HRESULT km_decoder_reset(void* value) {
    return protect([&] {
        auto& d = *static_cast<Decoder*>(value);
        check(d.decoder->ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0));
        if (d.converter) check(d.converter->ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0));
        check(d.decoder->ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0));
    });
}
HRESULT km_source_create(const wchar_t* path, void** result) { return protect([&] { *result = new Source(path); }); }
HRESULT km_source_next(void* value, Callback cb, void* user) { return protect([&] { static_cast<Source*>(value)->next(cb, user); }); }
void km_source_destroy(void* value) { delete static_cast<Source*>(value); }
}

extern "C" HRESULT km_audio_create(const wchar_t* path, void** result) {
    return protect([&] {
        auto source = std::make_unique<AudioSource>(path);
        *result = source->available ? source.release() : nullptr;
    });
}
extern "C" HRESULT km_audio_next(void* value, Callback cb, void* user) {
    return protect([&] { static_cast<AudioSource*>(value)->next(cb, user); });
}
extern "C" void km_audio_destroy(void* value) { delete static_cast<AudioSource*>(value); }
#include "windows_encoder.h"
