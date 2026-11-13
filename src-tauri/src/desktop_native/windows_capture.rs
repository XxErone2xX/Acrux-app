//! GPU capture fallback using a window-scoped Windows Graphics Capture item.
use super::{Image, Target};
use std::{
    thread,
    time::{Duration, Instant},
};
use windows::{
    core::Interface,
    Graphics::{
        Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
    },
    Win32::{
        Foundation::{HMODULE, HWND},
        Graphics::{Direct3D::D3D_DRIVER_TYPE_HARDWARE, Direct3D11::*, Dxgi::IDXGIDevice},
        System::WinRT::{
            Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess},
            Graphics::Capture::IGraphicsCaptureItemInterop,
        },
    },
};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn QueryPerformanceCounter(value: *mut i64) -> i32;
    fn QueryPerformanceFrequency(value: *mut i64) -> i32;
}
fn render_time(app_before: f64, counter: i64, frequency: i64, rendered_100ns: i64) -> Option<f64> {
    if !app_before.is_finite() || frequency <= 0 || counter < 0 || rendered_100ns < 0 {
        return None;
    }
    let age = counter as f64 / frequency as f64 - rendered_100ns as f64 / 10_000_000.;
    if !age.is_finite() || age < 0. {
        return None;
    }
    Some(app_before - age)
}

pub(super) struct Capture {
    target: Target,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    staging: Option<ID3D11Texture2D>,
    size: (u32, u32),
}
impl Capture {
    pub(super) fn matches(&self, target: Target) -> bool {
        self.target == target
    }
    pub(super) fn new(target: Target) -> Result<Self, String> {
        fn create(target: Target) -> windows::core::Result<Capture> {
            unsafe {
                if !GraphicsCaptureSession::IsSupported()? {
                    return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                        0x80004001u32 as i32,
                    )));
                }
                let (mut device, mut context) = (None, None);
                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_HARDWARE,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut context),
                )?;
                let device = device.ok_or_else(windows::core::Error::from_thread)?;
                let context = context.ok_or_else(windows::core::Error::from_thread)?;
                let dxgi: IDXGIDevice = device.cast()?;
                let direct: IDirect3DDevice =
                    CreateDirect3D11DeviceFromDXGIDevice(&dxgi)?.cast()?;
                let interop: IGraphicsCaptureItemInterop =
                    windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
                let item: GraphicsCaptureItem =
                    interop.CreateForWindow(HWND(target.id as *mut _))?;
                let size = item.Size()?;
                let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
                    &direct,
                    DirectXPixelFormat::B8G8R8A8UIntNormalized,
                    2,
                    size,
                )?;
                let session = pool.CreateCaptureSession(&item)?;
                session.SetIsCursorCaptureEnabled(false)?;
                session.StartCapture()?;
                Ok(Capture {
                    target,
                    device,
                    context,
                    pool,
                    session,
                    staging: None,
                    size: (0, 0),
                })
            }
        }
        create(target).map_err(|e| format!("Windows GPU 창 캡처를 시작하지 못했습니다: {e}"))
    }
    pub(super) fn image(&mut self, target: Target) -> Result<(Image, f64), String> {
        if self.target != target {
            return Err("GPU 캡처 대상이 바뀌었습니다.".into());
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        for _ in 0..2 {
            match self.pool.TryGetNextFrame() {
                Ok(old) => {
                    let _ = old.Close();
                }
                Err(_) => break,
            }
        }
        // Every delivered capture must be a newly dequeued WGC frame. No cached
        // pixels are presented as a fresh observation after an input command.
        let frame = loop {
            if super::super::STOP.load(std::sync::atomic::Ordering::Acquire) {
                return Err("게임 캡처를 취소했습니다.".into());
            }
            match self.pool.TryGetNextFrame() {
                Ok(frame) => break frame,
                Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(4)),
                Err(e) => return Err(format!("게임이 새 GPU 화면을 반환하지 않았습니다: {e}")),
            }
        };
        let captured = (|| -> windows::core::Result<(Image, f64)> {
            unsafe {
                // WGC supplies the compositor's QPC timestamp in 100ns units.
                // Sample app time before QPC: preemption can only make this
                // estimate older, never grant an old frame extra lifetime.
                let rendered = frame.SystemRelativeTime()?.Duration;
                let (mut frequency, mut counter) = (0, 0);
                if QueryPerformanceFrequency(&mut frequency) == 0 {
                    return Err(windows::core::Error::from_thread());
                }
                let app_before = crate::now();
                if QueryPerformanceCounter(&mut counter) == 0 {
                    return Err(windows::core::Error::from_thread());
                }
                let frame_at =
                    render_time(app_before, counter, frequency, rendered).ok_or_else(|| {
                        windows::core::Error::from_hresult(windows::core::HRESULT(
                            0x8007000du32 as i32,
                        ))
                    })?;
                let surface = frame.Surface()?;
                let access: IDirect3DDxgiInterfaceAccess = surface.cast()?;
                let texture: ID3D11Texture2D = access.GetInterface()?;
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                texture.GetDesc(&mut desc);
                let content = frame.ContentSize()?;
                if content.Width <= 0
                    || content.Height <= 0
                    || content.Width as u32 > desc.Width
                    || content.Height as u32 > desc.Height
                {
                    return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                        0x8007000du32 as i32,
                    )));
                }
                if self.size != (desc.Width, desc.Height) {
                    self.staging = None;
                    desc.Usage = D3D11_USAGE_STAGING;
                    desc.BindFlags = 0;
                    desc.MiscFlags = 0;
                    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                    self.device
                        .CreateTexture2D(&desc, None, Some(&mut self.staging))?;
                    self.size = (desc.Width, desc.Height);
                }
                let staging = self
                    .staging
                    .as_ref()
                    .ok_or_else(windows::core::Error::from_thread)?;
                self.context.CopyResource(staging, &texture);
                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                loop {
                    match self.context.Map(
                        staging,
                        0,
                        D3D11_MAP_READ,
                        D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
                        Some(&mut mapped),
                    ) {
                        Ok(()) => break,
                        Err(e) if e.code().0 as u32 == 0x887a000a && Instant::now() < deadline => {
                            thread::sleep(Duration::from_millis(2))
                        }
                        Err(e) => return Err(e),
                    }
                }
                let result = (|| {
                    let (x, y) = super::client_offset(
                        target,
                        (content.Width as usize, content.Height as usize),
                    )
                    .ok_or_else(windows::core::Error::from_thread)?;
                    let stride = mapped.RowPitch as usize;
                    if mapped.pData.is_null() || stride < (x + target.width) * 4 {
                        return Err(windows::core::Error::from_thread());
                    }
                    let mut data = Vec::with_capacity(target.width * target.height * 4);
                    for row in y..y + target.height {
                        data.extend_from_slice(std::slice::from_raw_parts(
                            (mapped.pData as *const u8).add(row * stride + x * 4),
                            target.width * 4,
                        ));
                    }
                    Ok(Image {
                        data,
                        width: target.width,
                        height: target.height,
                    })
                })();
                self.context.Unmap(staging, 0);
                result.map(|image| (image, frame_at))
            }
        })();
        let closed = frame.Close();
        let (image, at) = captured.map_err(|e| format!("게임 GPU 화면을 읽지 못했습니다: {e}"))?;
        closed.map_err(|e| format!("GPU 캡처 프레임을 해제하지 못했습니다: {e}"))?;
        image.validate()?;
        Ok((image, at))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compositor_time_keeps_old_frames_old_after_copying() {
        let at = render_time(10., 100_000, 1000, 995_000_000).unwrap();
        assert!((at - 9.5).abs() < 1e-9);
        assert!(render_time(10., 100_000, 1000, 1_001_000_000).is_none());
        assert!(render_time(10., 100_000, 0, 995_000_000).is_none());
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}
