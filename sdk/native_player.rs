// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// CPU callbacks appended to SDK-derived player definitions at build time.

pub use kyber_media::MediaError as PlayerError;

#[derive(Clone, Copy, Debug)]
pub struct CpuOutput {
    pub width: u32,
    pub height: u32,
    pub callback: unsafe extern "C" fn(*mut std::ffi::c_void, *const u8, usize, u32, u32, u64),
    pub audio: unsafe extern "C" fn(*mut std::ffi::c_void, *const u8, usize) -> i32,
    pub error: unsafe extern "C" fn(*mut std::ffi::c_void, *const u8, usize),
    pub userdata: usize,
}
impl CpuOutput {
    pub fn emit(self, frame: kyber_media::VideoFrame) {
        let mut rgba = vec![0; (self.width * self.height * 4) as usize];
        for y in 0..self.height {
            for x in 0..self.width {
                let from = (((y as u64 * frame.height as u64 / self.height as u64)
                    * frame.width as u64
                    + x as u64 * frame.width as u64 / self.width as u64)
                    * 4) as usize;
                let to = ((y * self.width + x) * 4) as usize;
                rgba[to..to + 4].copy_from_slice(&frame.rgba[from..from + 4]);
            }
        }
        unsafe {
            (self.callback)(
                self.userdata as _,
                rgba.as_ptr(),
                rgba.len(),
                self.width,
                self.height,
                frame.pts_us.max(0) as u64,
            );
        }
    }
    pub fn fail(self, message: &str) {
        let json = serde_json::json!({"kind": "error", "message": message}).to_string();
        unsafe {
            (self.error)(self.userdata as _, json.as_ptr(), json.len());
        }
    }
}
