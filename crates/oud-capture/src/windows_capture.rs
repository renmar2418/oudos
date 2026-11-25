//! Windows 10/11 screen capture (DXGI Desktop Duplication + GDI fallback) and
//! Media Foundation Hardware H.264 encoder discovery.

use std::time::Instant;

use windows::core::Interface;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIOutput1, IDXGIOutputDuplication, DXGI_ERROR_ACCESS_LOST,
    DXGI_ERROR_UNSUPPORTED, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO,
};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, MFMediaType_Video, MFShutdown, MFStartup, MFTEnumEx, MFT_FRIENDLY_NAME_Attribute,
    MFVideoFormat_H264, MFVideoFormat_NV12, MFSTARTUP_NOSOCKET, MFT_CATEGORY_VIDEO_ENCODER,
    MFT_ENUM_FLAG_HARDWARE, MFT_ENUM_FLAG_SORTANDFILTER, MFT_REGISTER_TYPE_INFO, MF_VERSION,
};
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_MULTITHREADED,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetDesktopWindow, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
};

use crate::capture::{DisplayGeometry, PixelFormat, RawFrame, Rect, ScreenCapturer};
use crate::error::{CaptureError, ScreenUnavailableReason};
use crate::CaptureBackend;

const E_ACCESSDENIED_HRESULT: i32 = 0x8007_0005_u32.cast_signed();
const DXGI_ERROR_NOT_FOUND_HRESULT: i32 = 0x887A_0002_u32.cast_signed();

/// Discovered Windows hardware H.264 Media Foundation Transform (NVENC / QSV / AMF).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareMftInfo {
    /// Human-readable friendly name reported by Media Foundation.
    pub friendly_name: String,
}

/// Probes Windows Media Foundation for hardware-accelerated H.264 encoders.
///
/// Returns an empty list if no hardware H.264 MFT is installed or available.
#[must_use]
pub fn probe_hardware_h264_mfts() -> Vec<HardwareMftInfo> {
    let mut found = Vec::new();
    // SAFETY: `CoInitializeEx` and `MFStartup` initialize COM/MF on the calling thread;
    // every allocated `IMFActivate` array returned by `MFTEnumEx` is freed via `CoTaskMemFree`.
    unsafe {
        let com_init = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        if MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).is_err() {
            if com_init {
                CoUninitialize();
            }
            return found;
        }

        let input_type = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Video,
            guidSubtype: MFVideoFormat_NV12,
        };
        let output_type = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Video,
            guidSubtype: MFVideoFormat_H264,
        };
        let mut activates: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut count: u32 = 0;

        let enum_res = MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER,
            Some(&raw const input_type),
            Some(&raw const output_type),
            &raw mut activates,
            &raw mut count,
        );

        if enum_res.is_ok() && !activates.is_null() {
            let slice = std::slice::from_raw_parts(activates, count as usize);
            for item in slice.iter().flatten() {
                let mut name_ptr = windows::core::PWSTR::null();
                let mut name_len = 0u32;
                if item
                    .GetAllocatedString(
                        &MFT_FRIENDLY_NAME_Attribute,
                        &raw mut name_ptr,
                        &raw mut name_len,
                    )
                    .is_ok()
                    && !name_ptr.is_null()
                {
                    let label = name_ptr
                        .to_string()
                        .unwrap_or_else(|_| "Hardware H.264 MFT".to_owned());
                    CoTaskMemFree(Some(name_ptr.0.cast()));
                    found.push(HardwareMftInfo {
                        friendly_name: label,
                    });
                }
            }
            CoTaskMemFree(Some(activates.cast()));
        }

        let _ = MFShutdown();
        if com_init {
            CoUninitialize();
        }
    }
    found
}

/// Windows Desktop Duplication capturer (`IDXGIOutputDuplication`) with automatic GDI fallback.
pub struct WindowsDesktopCapturer {
    dupl: Option<DxgiDuplicationState>,
    geometry: DisplayGeometry,
}

struct DxgiDuplicationState {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging: ID3D11Texture2D,
    width: u32,
    height: u32,
}

impl WindowsDesktopCapturer {
    /// Initializes Windows Desktop Duplication on primary display output 0,
    /// falling back to GDI screen capture if Desktop Duplication is unsupported
    /// in the current session.
    ///
    /// # Errors
    /// Returns [`CaptureError::ScreenUnavailable`] if no monitor is active or capture
    /// permissions are denied by the active desktop session.
    pub fn new() -> Result<Self, CaptureError> {
        match DxgiDuplicationState::try_init() {
            Ok((state, geom)) => Ok(Self {
                dupl: Some(state),
                geometry: geom,
            }),
            Err(CaptureError::ScreenUnavailable(ScreenUnavailableReason::NoDisplay)) => Err(
                CaptureError::ScreenUnavailable(ScreenUnavailableReason::NoDisplay),
            ),
            Err(_) => {
                let geom = probe_gdi_geometry()?;
                Ok(Self {
                    dupl: None,
                    geometry: geom,
                })
            }
        }
    }
}

impl ScreenCapturer for WindowsDesktopCapturer {
    fn backend(&self) -> CaptureBackend {
        CaptureBackend::WindowsWgc
    }

    fn probe_availability(&self) -> Result<DisplayGeometry, CaptureError> {
        if self.geometry.width == 0 || self.geometry.height == 0 {
            return Err(CaptureError::ScreenUnavailable(
                ScreenUnavailableReason::NoDisplay,
            ));
        }
        Ok(self.geometry.clone())
    }

    fn capture_frame(&mut self) -> Result<RawFrame, CaptureError> {
        if let Some(state) = &mut self.dupl {
            match state.capture_next_frame() {
                Ok(frame) => return Ok(frame),
                Err(CaptureError::ScreenUnavailable(ScreenUnavailableReason::Locked)) => {
                    return Err(CaptureError::ScreenUnavailable(
                        ScreenUnavailableReason::Locked,
                    ));
                }
                Err(_) => {
                    // Attempt GDI capture if DXGI duplication became temporarily unavailable.
                }
            }
        }
        capture_gdi_frame(&self.geometry)
    }
}

impl DxgiDuplicationState {
    fn try_init() -> Result<(Self, DisplayGeometry), CaptureError> {
        // SAFETY: All DXGI/D3D11 calls inspect return HRESULTs and hold valid COM references.
        unsafe {
            let mut device_opt: Option<ID3D11Device> = None;
            let mut context_opt: Option<ID3D11DeviceContext> = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                windows::Win32::Foundation::HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&raw mut device_opt),
                None,
                Some(&raw mut context_opt),
            )
            .map_err(|e| map_hresult(e.code().0))?;

            let device = device_opt
                .ok_or_else(|| CaptureError::CaptureFailed("missing ID3D11Device".to_owned()))?;
            let context = context_opt.ok_or_else(|| {
                CaptureError::CaptureFailed("missing ID3D11DeviceContext".to_owned())
            })?;

            let dxgi_device: IDXGIDevice = device
                .cast()
                .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;
            let adapter = dxgi_device
                .GetAdapter()
                .map_err(|e| map_hresult(e.code().0))?;
            let output = adapter
                .EnumOutputs(0)
                .map_err(|e| map_hresult(e.code().0))?;
            let out_desc = output.GetDesc().map_err(|e| map_hresult(e.code().0))?;
            let output1: IDXGIOutput1 = output
                .cast()
                .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;
            let duplication = output1
                .DuplicateOutput(&device)
                .map_err(|e| map_hresult(e.code().0))?;

            let dupl_desc = duplication.GetDesc();
            let width = dupl_desc.ModeDesc.Width.max(1);
            let height = dupl_desc.ModeDesc.Height.max(1);

            let staging_desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };
            let mut staging_opt: Option<ID3D11Texture2D> = None;
            device
                .CreateTexture2D(&raw const staging_desc, None, Some(&raw mut staging_opt))
                .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;
            let staging = staging_opt.ok_or_else(|| {
                CaptureError::CaptureFailed("failed to allocate DXGI staging texture".to_owned())
            })?;

            let monitor_name = String::from_utf16_lossy(
                &out_desc
                    .DeviceName
                    .iter()
                    .copied()
                    .take_while(|&c| c != 0)
                    .collect::<Vec<_>>(),
            );
            let refresh_hz = dupl_desc
                .ModeDesc
                .RefreshRate
                .Numerator
                .checked_div(dupl_desc.ModeDesc.RefreshRate.Denominator)
                .unwrap_or(60);

            let geom = DisplayGeometry {
                width,
                height,
                refresh_hz: refresh_hz.max(30),
                monitor_name,
            };

            Ok((
                Self {
                    device,
                    context,
                    duplication,
                    staging,
                    width,
                    height,
                },
                geom,
            ))
        }
    }

    fn capture_next_frame(&mut self) -> Result<RawFrame, CaptureError> {
        // SAFETY: `AcquireNextFrame` and `Map`/`Unmap` are paired and released before returning.
        unsafe {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource_opt = None;
            if let Err(err) =
                self.duplication
                    .AcquireNextFrame(32, &raw mut frame_info, &raw mut resource_opt)
            {
                let code = err.code();
                if code == DXGI_ERROR_WAIT_TIMEOUT {
                    return Err(CaptureError::CaptureFailed(
                        "DXGI frame wait timeout".to_owned(),
                    ));
                }
                return Err(map_hresult(code.0));
            }

            let mut dirty_rects = Vec::new();
            if frame_info.TotalMetadataBufferSize > 0 {
                let mut rects = vec![RECT::default(); 64];
                let mut required = 0u32;
                let buf_bytes =
                    u32::try_from(rects.len() * std::mem::size_of::<RECT>()).unwrap_or(u32::MAX);
                if self
                    .duplication
                    .GetFrameDirtyRects(buf_bytes, rects.as_mut_ptr(), &raw mut required)
                    .is_ok()
                {
                    let count = (required as usize) / std::mem::size_of::<RECT>();
                    for r in rects.into_iter().take(count) {
                        dirty_rects.push(Rect {
                            x: r.left,
                            y: r.top,
                            width: u32::try_from(r.right - r.left).unwrap_or(0),
                            height: u32::try_from(r.bottom - r.top).unwrap_or(0),
                        });
                    }
                }
            }

            let resource = resource_opt.ok_or_else(|| {
                let _ = self.duplication.ReleaseFrame();
                CaptureError::CaptureFailed("DXGI returned empty desktop resource".to_owned())
            })?;
            let src_tex: ID3D11Texture2D = match resource.cast() {
                Ok(t) => t,
                Err(e) => {
                    let _ = self.duplication.ReleaseFrame();
                    return Err(CaptureError::CaptureFailed(e.to_string()));
                }
            };

            self.context.CopyResource(&self.staging, &src_tex);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            if let Err(e) =
                self.context
                    .Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&raw mut mapped))
            {
                let _ = self.duplication.ReleaseFrame();
                return Err(CaptureError::CaptureFailed(e.to_string()));
            }

            let row_bytes = (self.width as usize) * 4;
            let pitch = mapped.RowPitch as usize;
            let mut data = vec![0u8; row_bytes * (self.height as usize)];
            for y in 0..(self.height as usize) {
                let src_row =
                    std::slice::from_raw_parts(mapped.pData.cast::<u8>().add(y * pitch), row_bytes);
                let dst_start = y * row_bytes;
                data[dst_start..dst_start + row_bytes].copy_from_slice(src_row);
            }

            self.context.Unmap(&self.staging, 0);
            let _ = self.duplication.ReleaseFrame();
            let _ = &self.device;

            Ok(RawFrame {
                width: self.width,
                height: self.height,
                stride: row_bytes,
                pixel_format: PixelFormat::Bgra8,
                captured_at: Instant::now(),
                dirty_rects,
                data,
            })
        }
    }
}

fn probe_gdi_geometry() -> Result<DisplayGeometry, CaptureError> {
    // SAFETY: `GetSystemMetrics` is a read-only Win32 display dimension query.
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        if w <= 0 || h <= 0 {
            return Err(CaptureError::ScreenUnavailable(
                ScreenUnavailableReason::NoDisplay,
            ));
        }
        Ok(DisplayGeometry {
            width: u32::try_from(w).unwrap_or(1920),
            height: u32::try_from(h).unwrap_or(1080),
            refresh_hz: 60,
            monitor_name: "GDI-Primary-Display".to_owned(),
        })
    }
}

fn capture_gdi_frame(geometry: &DisplayGeometry) -> Result<RawFrame, CaptureError> {
    // SAFETY: GDI DC/HBITMAP handles are explicitly released and deleted before return.
    unsafe {
        let hwnd = GetDesktopWindow();
        let hdc_screen = GetDC(Some(hwnd));
        if hdc_screen.is_invalid() {
            return Err(CaptureError::ScreenUnavailable(
                ScreenUnavailableReason::PermissionMissing,
            ));
        }

        let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
        let width_i32 = i32::try_from(geometry.width).unwrap_or(1920);
        let height_i32 = i32::try_from(geometry.height).unwrap_or(1080);
        let hbitmap = CreateCompatibleBitmap(hdc_screen, width_i32, height_i32);

        if hdc_mem.is_invalid() || hbitmap.is_invalid() {
            if !hbitmap.is_invalid() {
                let _ = DeleteObject(hbitmap.into());
            }
            if !hdc_mem.is_invalid() {
                let _ = DeleteDC(hdc_mem);
            }
            let _ = ReleaseDC(Some(hwnd), hdc_screen);
            return Err(CaptureError::ScreenUnavailable(
                ScreenUnavailableReason::PermissionMissing,
            ));
        }

        let old = SelectObject(hdc_mem, hbitmap.into());
        let blt_ok = BitBlt(
            hdc_mem,
            0,
            0,
            width_i32,
            height_i32,
            Some(hdc_screen),
            0,
            0,
            SRCCOPY,
        )
        .is_ok();

        let stride = (geometry.width as usize) * 4;
        let mut pixels = vec![0u8; stride * (geometry.height as usize)];

        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: u32::try_from(std::mem::size_of::<BITMAPINFOHEADER>()).unwrap_or(40),
                biWidth: width_i32,
                biHeight: -height_i32, // Top-down BGRA scanlines.
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let copied = if blt_ok {
            GetDIBits(
                hdc_mem,
                hbitmap,
                0,
                geometry.height,
                Some(pixels.as_mut_ptr().cast()),
                &raw mut bmi,
                DIB_RGB_COLORS,
            )
        } else {
            0
        };

        SelectObject(hdc_mem, old);
        let _ = DeleteObject(hbitmap.into());
        let _ = DeleteDC(hdc_mem);
        let _ = ReleaseDC(Some(hwnd), hdc_screen);

        if !blt_ok || copied == 0 {
            return Err(CaptureError::ScreenUnavailable(
                ScreenUnavailableReason::Locked,
            ));
        }

        Ok(RawFrame {
            width: geometry.width,
            height: geometry.height,
            stride,
            pixel_format: PixelFormat::Bgra8,
            captured_at: Instant::now(),
            dirty_rects: vec![Rect {
                x: 0,
                y: 0,
                width: geometry.width,
                height: geometry.height,
            }],
            data: pixels,
        })
    }
}

fn map_hresult(code: i32) -> CaptureError {
    if code == DXGI_ERROR_ACCESS_LOST.0 || code == E_ACCESSDENIED_HRESULT {
        CaptureError::ScreenUnavailable(ScreenUnavailableReason::Locked)
    } else if code == DXGI_ERROR_NOT_FOUND_HRESULT {
        CaptureError::ScreenUnavailable(ScreenUnavailableReason::NoDisplay)
    } else if code == DXGI_ERROR_UNSUPPORTED.0 {
        CaptureError::ScreenUnavailable(ScreenUnavailableReason::PermissionMissing)
    } else {
        CaptureError::CaptureFailed(format!("Win32/DXGI HRESULT 0x{code:08X}"))
    }
}
