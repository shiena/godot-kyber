// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

//! D3D12 output for a renderer-owned device. Textures are immutable after decoding.
use crate::{EncodedPacket, MediaError, Result};
use std::{ffi::c_void, marker::PhantomData, rc::Rc};
type Callback = unsafe extern "C" fn(*mut c_void, *mut c_void, u32, u32, i64);
unsafe extern "C" {
    fn km_gpu_context_create(device: *mut c_void, result: *mut *mut c_void) -> i32;
    fn km_gpu_context_destroy(context: *mut c_void);
    fn km_gpu_decoder_create(context: *mut c_void, result: *mut *mut c_void) -> i32;
    fn km_decoder_destroy(decoder: *mut c_void);
    fn km_gpu_decoder_push(
        decoder: *mut c_void,
        data: *const u8,
        len: usize,
        pts: i64,
        callback: Callback,
        user: *mut c_void,
    ) -> i32;
    fn km_gpu_frame_destroy(frame: *mut c_void);
    fn km_gpu_frame_resource(frame: *mut c_void) -> *mut c_void;
}
fn check(code: i32) -> Result<()> {
    if code < 0 {
        Err(MediaError(format!(
            "D3D12 texture sharing failed: 0x{:08X}",
            code as u32
        )))
    } else {
        Ok(())
    }
}
pub struct Context(*mut c_void);
// Ownership moves from the render thread to exactly one codec worker.
unsafe impl Send for Context {}
impl Context {
    /// # Safety
    /// `device` must be a live ID3D12Device belonging to the Godot renderer.
    pub unsafe fn new(device: u64) -> Result<Self> {
        if device == 0 {
            return Err(MediaError("Missing D3D12 device".into()));
        }
        let mut handle = std::ptr::null_mut();
        check(unsafe { km_gpu_context_create(device as *mut c_void, &mut handle) })?;
        Ok(Self(handle))
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        unsafe { km_gpu_context_destroy(self.0) };
    }
}
#[derive(Debug)]
pub struct Frame {
    handle: *mut c_void,
    pub width: u32,
    pub height: u32,
    pub pts_us: i64,
}
// Completed D3D12 resources are immutable; COM reference counting is thread-safe.
unsafe impl Send for Frame {}
unsafe impl Sync for Frame {}
impl Frame {
    pub fn resource(&self) -> u64 {
        unsafe { km_gpu_frame_resource(self.handle) as u64 }
    }
}
impl Drop for Frame {
    fn drop(&mut self) {
        unsafe { km_gpu_frame_destroy(self.handle) };
    }
}
pub struct Decoder {
    handle: *mut c_void,
    _context: Context,
    _thread: PhantomData<Rc<()>>,
}
impl Decoder {
    pub fn new(context: Context) -> Result<Self> {
        let mut handle = std::ptr::null_mut();
        check(unsafe { km_gpu_decoder_create(context.0, &mut handle) })?;
        Ok(Self {
            handle,
            _context: context,
            _thread: PhantomData,
        })
    }
    pub fn decode(&mut self, packet: EncodedPacket<'_>) -> Result<Vec<Frame>> {
        unsafe extern "C" fn collect(
            user: *mut c_void,
            handle: *mut c_void,
            width: u32,
            height: u32,
            pts_us: i64,
        ) {
            unsafe { &mut *user.cast::<Vec<Frame>>() }.push(Frame {
                handle,
                width,
                height,
                pts_us,
            });
        }
        let mut frames = Vec::new();
        check(unsafe {
            km_gpu_decoder_push(
                self.handle,
                packet.data.as_ptr(),
                packet.data.len(),
                packet.pts_us,
                collect,
                (&mut frames as *mut Vec<Frame>).cast(),
            )
        })?;
        Ok(frames)
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe { km_decoder_destroy(self.handle) };
    }
}
