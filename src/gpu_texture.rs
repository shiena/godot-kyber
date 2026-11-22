// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

//! D3D12 texture import on Godot's render thread, with deferred external ownership.
use crate::frame::Mailbox;
use godot::{
    classes::rendering_device as rd,
    classes::{RdTextureFormat, RdTextureView, RenderingServer, Texture2D, Texture2Drd},
    prelude::*,
};
use std::{
    cell::RefCell,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
type NativeFrame = Arc<kyber_media::gpu::Frame>;
struct Token {
    rid: Rid,
    frame: NativeFrame,
}
#[derive(Default)]
struct State {
    closed: bool,
    ready: Option<Token>,
    output: Option<Rid>,
}
thread_local! {
    static RETIRED: RefCell<(u64, Vec<(u64, NativeFrame)>)> = const { RefCell::new((0, Vec::new())) };
    static RETIRING: RefCell<Vec<(u64, Token)>> = const { RefCell::new(Vec::new()) };
}
static CONNECTED: AtomicBool = AtomicBool::new(false);
fn retire_here(token: Token) {
    if let Some(device) = RenderingServer::singleton().get_rendering_device() {
        let delay = u64::from(device.get_frame_delay()) + 2;
        let due = RETIRED.with_borrow(|state| state.0 + delay);
        RETIRING.with_borrow_mut(|pending| pending.push((due, token)));
    }
}
fn retire(token: Token) {
    let mut pending = Some(token);
    RenderingServer::singleton().call_on_render_thread(&Callable::from_sync_fn(
        "kyber_gpu_retire",
        move |_| {
            if let Some(token) = pending.take() {
                retire_here(token);
            }
        },
    ));
}
pub fn request(mailbox: Arc<Mailbox>) {
    let mut server = RenderingServer::singleton();
    // Older Godot versions and other graphics backends retain the CPU path.
    if !server.has_method("get_current_rendering_driver_name")
        || server
            .call("get_current_rendering_driver_name", &[])
            .to::<GString>()
            != "d3d12"
    {
        return;
    }
    if !CONNECTED.swap(true, Ordering::AcqRel) {
        server.connect(
            "frame_post_draw",
            &Callable::from_sync_fn("kyber_gpu_collect", |_| {
                RenderingServer::singleton().call_on_render_thread(&Callable::from_sync_fn(
                    "kyber_gpu_release",
                    |_| {
                        RETIRED.with_borrow_mut(|state| {
                            state.0 += 1;
                            let now = state.0;
                            state.1.retain(|(due, _)| *due > now);
                            if let Some(mut device) =
                                RenderingServer::singleton().get_rendering_device()
                            {
                                let delay = u64::from(device.get_frame_delay()) + 2;
                                RETIRING.with_borrow_mut(|pending| {
                                    let mut index = 0;
                                    while index < pending.len() {
                                        if pending[index].0 > now {
                                            index += 1;
                                            continue;
                                        }
                                        let (_, token) = pending.swap_remove(index);
                                        if device.texture_is_valid(token.rid) {
                                            device.free_rid(token.rid);
                                        }
                                        state.1.push((now + delay, token.frame));
                                    }
                                });
                            }
                        });
                    },
                ));
            }),
        );
    }
    mailbox.gpu_pending.store(true, Ordering::Release);
    server.call_on_render_thread(&Callable::from_sync_fn("kyber_gpu_device", move |_| {
        if let Some(device) = RenderingServer::singleton().get_rendering_device() {
            let native = device.get_driver_resource(rd::DriverResource::DEVICE, Rid::Invalid, 0);
            // The driver name was checked before interpreting the native device.
            match unsafe { kyber_media::gpu::Context::new(native) } {
                Ok(context) => *mailbox.gpu_context.lock().unwrap() = Some(context),
                Err(error) => *mailbox.gpu_status.lock().unwrap() = error.to_string(),
            }
        }
        mailbox.gpu_pending.store(false, Ordering::Release);
    }));
}
#[derive(Default)]
pub struct Presenter {
    state: Arc<Mutex<State>>,
    current: Option<Token>,
    texture: Option<Gd<Texture2Drd>>,
}
impl Presenter {
    pub fn submit(&mut self, frame: NativeFrame) {
        let state = self.state.clone();
        RenderingServer::singleton().call_on_render_thread(&Callable::from_sync_fn(
            "kyber_gpu_import",
            move |_| {
                let mut state = state.lock().unwrap();
                if state.closed {
                    return;
                }
                let Some(mut device) = RenderingServer::singleton().get_rendering_device() else {
                    return;
                };
                let rid = device.texture_create_from_extension(
                    rd::TextureType::TYPE_2D,
                    rd::DataFormat::R8G8B8A8_UNORM,
                    rd::TextureSamples::SAMPLES_1,
                    rd::TextureUsageBits::SAMPLING_BIT
                        | rd::TextureUsageBits::CAN_COPY_FROM_BIT
                        | rd::TextureUsageBits::COLOR_ATTACHMENT_BIT,
                    frame.resource(),
                    u64::from(frame.width),
                    u64::from(frame.height),
                    1,
                    1,
                );
                if !rid.is_valid() {
                    return;
                }
                // Godot's D3D12 texture views require an engine-owned allocation.
                // Copy on the GPU; no pixel data crosses through CPU memory.
                let mut format = RdTextureFormat::new_gd();
                format.set_format(rd::DataFormat::R8G8B8A8_UNORM);
                format.set_width(frame.width);
                format.set_height(frame.height);
                format.set_usage_bits(
                    rd::TextureUsageBits::SAMPLING_BIT
                        | rd::TextureUsageBits::CAN_COPY_TO_BIT
                        | rd::TextureUsageBits::CAN_COPY_FROM_BIT,
                );
                let owned = if let Some(output) = state.output {
                    output
                } else {
                    let output = device.texture_create(&format, &RdTextureView::new_gd());
                    if output.is_valid() {
                        state.output = Some(output);
                    }
                    output
                };
                if !owned.is_valid() {
                    retire_here(Token {
                        rid,
                        frame: frame.clone(),
                    });
                    return;
                }
                let result = device.texture_copy(
                    rid,
                    owned,
                    Vector3::ZERO,
                    Vector3::ZERO,
                    Vector3::new(frame.width as f32, frame.height as f32, 1.0),
                    0,
                    0,
                    0,
                    0,
                );
                retire_here(Token {
                    rid,
                    frame: frame.clone(),
                });
                if result != godot::global::Error::OK {
                    return;
                }
                let token = Token {
                    rid: owned,
                    frame: frame.clone(),
                };
                state.ready = Some(token);
            },
        ));
    }
    pub fn pump(&mut self) -> bool {
        let ready = self.state.lock().unwrap().ready.take();
        let Some(ready) = ready else {
            return false;
        };
        let texture = self.texture.get_or_insert_with(Texture2Drd::new_gd);
        if self.current.is_none() {
            texture.set_texture_rd_rid(ready.rid);
        }
        self.current = Some(ready);
        true
    }
    pub fn texture(&self) -> Option<Gd<Texture2D>> {
        self.texture.clone().map(|t| t.upcast())
    }
}
impl Drop for Presenter {
    fn drop(&mut self) {
        if let Some(texture) = &mut self.texture {
            texture.set_texture_rd_rid(Rid::Invalid);
        }
        let mut state = self.state.lock().unwrap();
        state.closed = true;
        let pending = state.ready.take();
        if let Some(token) = self.current.take().or(pending) {
            retire(token);
        }
    }
}
