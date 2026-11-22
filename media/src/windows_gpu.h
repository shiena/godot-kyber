// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// Immutable D3D12 textures shared with a D3D11 video processor on the same adapter.
#include <d3d12.h>
#include <d3d11_1.h>
#include <dxgi1_4.h>
struct GpuContext {
    ComPtr<ID3D12Device> renderer;
    ComPtr<ID3D12CommandQueue> queue;
    ComPtr<ID3D12CommandAllocator> allocator;
    ComPtr<ID3D12GraphicsCommandList> commands;
    ComPtr<ID3D12Fence> fence;
    ComPtr<ID3D11Device> device;
    ComPtr<ID3D11DeviceContext> context;
    ComPtr<ID3D11VideoDevice> video;
    ComPtr<ID3D11VideoContext> video_context;
    UINT64 serial = 0;
    explicit GpuContext(ID3D12Device* target) : renderer(target) {
        static SystemLibrary dxgi{L"dxgi.dll"}, d3d{L"d3d11.dll"};
        auto factory_fn = dxgi.get<decltype(&CreateDXGIFactory1)>("CreateDXGIFactory1");
        ComPtr<IDXGIFactory4> factory;
        check(factory_fn(IID_PPV_ARGS(&factory)));
        ComPtr<IDXGIAdapter> adapter;
        check(factory->EnumAdapterByLuid(renderer->GetAdapterLuid(), IID_PPV_ARGS(&adapter)));
        auto create = d3d.get<decltype(&D3D11CreateDevice)>("D3D11CreateDevice");
        check(create(adapter.Get(), D3D_DRIVER_TYPE_UNKNOWN, nullptr,
            D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            nullptr, 0, D3D11_SDK_VERSION, &device, nullptr, &context));
        ComPtr<ID3D10Multithread> multithread;
        check(device.As(&multithread)); multithread->SetMultithreadProtected(TRUE);
        check(device.As(&video)); check(context.As(&video_context));
        D3D12_COMMAND_QUEUE_DESC desc{};
        check(renderer->CreateCommandQueue(&desc, IID_PPV_ARGS(&queue)));
        check(renderer->CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT, IID_PPV_ARGS(&allocator)));
        check(renderer->CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, allocator.Get(), nullptr, IID_PPV_ARGS(&commands)));
        check(commands->Close());
        check(renderer->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&fence)));
    }
    void ready(ID3D12Resource* resource) {
        check(allocator->Reset()); check(commands->Reset(allocator.Get(), nullptr));
        D3D12_RESOURCE_BARRIER barrier{};
        barrier.Type = D3D12_RESOURCE_BARRIER_TYPE_TRANSITION;
        barrier.Transition.pResource = resource;
        barrier.Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
        barrier.Transition.StateBefore = D3D12_RESOURCE_STATE_COMMON;
        // Godot's D3D12 extension import starts tracking textures as render targets.
        barrier.Transition.StateAfter = D3D12_RESOURCE_STATE_RENDER_TARGET;
        commands->ResourceBarrier(1, &barrier);
        check(commands->Close());
        ID3D12CommandList* lists[] = {commands.Get()};
        queue->ExecuteCommandLists(1, lists);
        check(queue->Signal(fence.Get(), ++serial));
        HANDLE event = CreateEventW(nullptr, FALSE, FALSE, nullptr);
        if (!event) throw Failure{HRESULT_FROM_WIN32(GetLastError())};
        HRESULT hr = fence->SetEventOnCompletion(serial, event);
        DWORD result = SUCCEEDED(hr) ? WaitForSingleObject(event, 5000) : WAIT_FAILED;
        CloseHandle(event); check(hr);
        if (result != WAIT_OBJECT_0) throw Failure{HRESULT_FROM_WIN32(WAIT_TIMEOUT)};
    }
};
struct GpuFrame {
    ComPtr<ID3D12Resource> resource;
    UINT width, height;
};
using GpuCallback = void (*)(void*, void*, uint32_t, uint32_t, int64_t);
static std::unique_ptr<GpuFrame> convert_gpu(GpuContext& gpu, ID3D11Texture2D* input, UINT slice, IMFMediaType* type) {
    auto result = std::make_unique<GpuFrame>();
    UINT coded_w, coded_h;
    check(MFGetAttributeSize(type, MF_MT_FRAME_SIZE, &coded_w, &coded_h));
    result->width = coded_w; result->height = coded_h;
    LONG left = 0, top = 0;
    MFVideoArea aperture{};
    if (SUCCEEDED(type->GetBlob(MF_MT_MINIMUM_DISPLAY_APERTURE, reinterpret_cast<BYTE*>(&aperture), sizeof(aperture), nullptr)) ||
        SUCCEEDED(type->GetBlob(MF_MT_GEOMETRIC_APERTURE, reinterpret_cast<BYTE*>(&aperture), sizeof(aperture), nullptr))) {
        left = aperture.OffsetX.value; top = aperture.OffsetY.value;
        if (left < 0 || top < 0 || aperture.Area.cx <= 0 || aperture.Area.cy <= 0 ||
            UINT64(left) + aperture.Area.cx > coded_w || UINT64(top) + aperture.Area.cy > coded_h) throw Failure{MF_E_INVALIDMEDIATYPE};
        result->width = aperture.Area.cx; result->height = aperture.Area.cy;
    }
    D3D12_HEAP_PROPERTIES heap{}; heap.Type = D3D12_HEAP_TYPE_DEFAULT;
    D3D12_RESOURCE_DESC desc{};
    desc.Dimension = D3D12_RESOURCE_DIMENSION_TEXTURE2D;
    desc.Width = result->width; desc.Height = result->height; desc.DepthOrArraySize = 1; desc.MipLevels = 1;
    desc.Format = DXGI_FORMAT_R8G8B8A8_UNORM; desc.SampleDesc.Count = 1;
    desc.Flags = D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    check(gpu.renderer->CreateCommittedResource(&heap, D3D12_HEAP_FLAG_SHARED, &desc,
        D3D12_RESOURCE_STATE_COMMON, nullptr, IID_PPV_ARGS(&result->resource)));
    HANDLE handle = nullptr;
    check(gpu.renderer->CreateSharedHandle(result->resource.Get(), nullptr, GENERIC_ALL, nullptr, &handle));
    ComPtr<ID3D11Device1> device1;
    HRESULT hr = gpu.device.As(&device1);
    ComPtr<ID3D11Texture2D> target;
    if (SUCCEEDED(hr)) hr = device1->OpenSharedResource1(handle, IID_PPV_ARGS(&target));
    CloseHandle(handle); check(hr);
    D3D11_VIDEO_PROCESSOR_CONTENT_DESC content{};
    content.InputFrameFormat = D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE;
    content.InputWidth = coded_w; content.InputHeight = coded_h;
    content.OutputWidth = result->width; content.OutputHeight = result->height;
    content.Usage = D3D11_VIDEO_USAGE_OPTIMAL_SPEED;
    ComPtr<ID3D11VideoProcessorEnumerator> enumeration;
    check(gpu.video->CreateVideoProcessorEnumerator(&content, &enumeration));
    ComPtr<ID3D11VideoProcessor> processor;
    check(gpu.video->CreateVideoProcessor(enumeration.Get(), 0, &processor));
    D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC in{};
    in.ViewDimension = D3D11_VPIV_DIMENSION_TEXTURE2D; in.Texture2D.ArraySlice = slice;
    ComPtr<ID3D11VideoProcessorInputView> input_view;
    check(gpu.video->CreateVideoProcessorInputView(input, enumeration.Get(), &in, &input_view));
    D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC out{}; out.ViewDimension = D3D11_VPOV_DIMENSION_TEXTURE2D;
    ComPtr<ID3D11VideoProcessorOutputView> output_view;
    check(gpu.video->CreateVideoProcessorOutputView(target.Get(), enumeration.Get(), &out, &output_view));
    RECT source{left, top, left + LONG(result->width), top + LONG(result->height)};
    RECT destination{0, 0, LONG(result->width), LONG(result->height)};
    gpu.video_context->VideoProcessorSetStreamSourceRect(processor.Get(), 0, TRUE, &source);
    gpu.video_context->VideoProcessorSetStreamDestRect(processor.Get(), 0, TRUE, &destination);
    gpu.video_context->VideoProcessorSetOutputTargetRect(processor.Get(), TRUE, &destination);
    D3D11_VIDEO_PROCESSOR_COLOR_SPACE yuv{}; // BT.601 limited-range input for the addon format.
    D3D11_VIDEO_PROCESSOR_COLOR_SPACE rgb{}; rgb.RGB_Range = 0;
    gpu.video_context->VideoProcessorSetStreamColorSpace(processor.Get(), 0, &yuv);
    gpu.video_context->VideoProcessorSetOutputColorSpace(processor.Get(), &rgb);
    D3D11_VIDEO_PROCESSOR_STREAM stream{}; stream.Enable = TRUE; stream.pInputSurface = input_view.Get();
    check(gpu.video_context->VideoProcessorBlt(processor.Get(), output_view.Get(), 0, 1, &stream));
    D3D11_QUERY_DESC query_desc{D3D11_QUERY_EVENT, 0};
    ComPtr<ID3D11Query> query; check(gpu.device->CreateQuery(&query_desc, &query));
    gpu.context->End(query.Get()); gpu.context->Flush();
    ULONGLONG deadline = GetTickCount64() + 5000;
    while ((hr = gpu.context->GetData(query.Get(), nullptr, 0, 0)) == S_FALSE) {
        if (GetTickCount64() >= deadline) throw Failure{HRESULT_FROM_WIN32(WAIT_TIMEOUT)};
        Sleep(1);
    }
    check(hr);
    gpu.ready(result->resource.Get());
    return result;
}
