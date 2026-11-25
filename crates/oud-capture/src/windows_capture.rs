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

struct ComMfGuard {
    com_init: bool,
    mf_started: bool,
}

impl Drop for ComMfGuard {
    fn drop(&mut self) {
        // SAFETY: `MFShutdown` pairs with a successful `MFStartup`, and `CoUninitialize`
        // pairs strictly with `CoInitializeEx` returning `S_OK` / `S_FALSE`.
        unsafe {
            if self.mf_started {
                let _ = MFShutdown();
            }
            if self.com_init {
                CoUninitialize();
            }
        }
    }
}

struct MftActivateArray {
    ptr: *mut Option<IMFActivate>,
    count: usize,
}

impl Drop for MftActivateArray {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY: `ptr` is the CoTaskMem-allocated array returned by `MFTEnumEx` containing
            // `count` `Option<IMFActivate>` COM pointers; taking and dropping each element releases
            // its COM reference before freeing the outer buffer.
            unsafe {
                for idx in 0..self.count {
                    let _ = std::ptr::read(self.ptr.add(idx));
                }
                CoTaskMemFree(Some(self.ptr.cast()));
            }
        }
    }
}

struct CoTaskMemPwstr(windows::core::PWSTR);

impl Drop for CoTaskMemPwstr {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: `self.0` was allocated by `IMFActivate::GetAllocatedString`.
            unsafe {
                CoTaskMemFree(Some(self.0 .0.cast()));
            }
        }
    }
}

/// Probes Windows Media Foundation for hardware-accelerated H.264 encoders.
///
/// Returns an empty list if no hardware H.264 MFT is installed or available.
#[must_use]
pub fn probe_hardware_h264_mfts() -> Vec<HardwareMftInfo> {
    let mut found = Vec::new();
    // SAFETY: COM/MF initialization and enumerated `IMFActivate` pointers are owned by
    // RAII scope guards (`ComMfGuard` and `MftActivateArray`), declared so COM objects
    // release before `MFShutdown` and `CoUninitialize`.
    unsafe {
        let mut guard = ComMfGuard {
            com_init: CoInitializeEx(None, COINIT_MULTITHREADED).is_ok(),
            mf_started: false,
        };
        if MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).is_err() {
            return found;
        }
        guard.mf_started = true;

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
            let activate_array = MftActivateArray {
                ptr: activates,
                count: count as usize,
            };
            let slice = std::slice::from_raw_parts(activate_array.ptr, activate_array.count);
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
                    let name_guard = CoTaskMemPwstr(name_ptr);
                    let label = name_guard
                        .0
                        .to_string()
                        .unwrap_or_else(|_| "Hardware H.264 MFT".to_owned());
                    found.push(HardwareMftInfo {
                        friendly_name: label,
                    });
                }
            }
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

struct DxgiFrameGuard<'a> {
    duplication: &'a IDXGIOutputDuplication,
}

impl Drop for DxgiFrameGuard<'_> {
    fn drop(&mut self) {
        // SAFETY: Constructed only after a successful `AcquireNextFrame`.
        unsafe {
            let _ = self.duplication.ReleaseFrame();
        }
    }
}

struct StagingMapGuard<'a> {
    context: &'a ID3D11DeviceContext,
    staging: &'a ID3D11Texture2D,
}

impl Drop for StagingMapGuard<'_> {
    fn drop(&mut self) {
        // SAFETY: Constructed only after a successful `ID3D11DeviceContext::Map`.
        unsafe {
            self.context.Unmap(self.staging, 0);
        }
    }
}

impl WindowsDesktopCapturer {
    /// Initializes Windows Desktop Duplication on primary display output 0,
    /// falling back to GDI screen capture only if DXGI initialization fails with a
    /// non-availability driver error ([`CaptureError::CaptureFailed`]).
    ///
    /// Propagates [`CaptureError::ScreenUnavailable`] (`Locked`, `PermissionMissing`,
    /// `NoDisplay`, `Asleep`) directly instead of masking OS restrictions with GDI.
    ///
    /// # Errors
    /// Returns [`CaptureError::ScreenUnavailable`] if locked, asleep, headless, or lacking
    /// display capture permissions.
    pub fn new() -> Result<Self, CaptureError> {
        match resolve_dxgi_init(DxgiDuplicationState::try_init(), probe_gdi_geometry) {
            Ok((Some(state), geometry)) => Ok(Self {
                dupl: Some(state),
                geometry,
            }),
            Ok((None, geometry)) => Ok(Self {
                dupl: None,
                geometry,
            }),
            Err(err) => Err(err),
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
            return route_dxgi_capture(state.capture_next_frame(), || {
                capture_gdi_frame(&self.geometry)
            });
        }
        capture_gdi_frame(&self.geometry)
    }
}

fn resolve_dxgi_init<S, F>(
    dxgi_result: Result<(S, DisplayGeometry), CaptureError>,
    gdi_probe: F,
) -> Result<(Option<S>, DisplayGeometry), CaptureError>
where
    F: FnOnce() -> Result<DisplayGeometry, CaptureError>,
{
    match dxgi_result {
        Ok((state, geom)) => Ok((Some(state), geom)),
        Err(err @ CaptureError::ScreenUnavailable(_)) => Err(err),
        Err(_) => {
            let geom = gdi_probe()?;
            Ok((None, geom))
        }
    }
}

fn route_dxgi_capture<F>(
    dxgi_frame: Result<RawFrame, CaptureError>,
    gdi_capture: F,
) -> Result<RawFrame, CaptureError>
where
    F: FnOnce() -> Result<RawFrame, CaptureError>,
{
    match dxgi_frame {
        Ok(frame) => Ok(frame),
        Err(err @ CaptureError::ScreenUnavailable(_)) => Err(err),
        Err(_) => gdi_capture(),
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
            let width = dupl_desc.ModeDesc.Width;
            let height = dupl_desc.ModeDesc.Height;
            if width == 0 || height == 0 {
                return Err(CaptureError::ScreenUnavailable(
                    ScreenUnavailableReason::NoDisplay,
                ));
            }

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
            let frame_guard = DxgiFrameGuard {
                duplication: &self.duplication,
            };

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
                        if let Some(dirty) =
                            dirty_rect_from_bounds(r.left, r.top, r.right, r.bottom)
                        {
                            dirty_rects.push(dirty);
                        }
                    }
                }
            }

            let resource = resource_opt.ok_or_else(|| {
                CaptureError::CaptureFailed("DXGI returned empty desktop resource".to_owned())
            })?;
            let src_tex: ID3D11Texture2D = resource
                .cast()
                .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;

            self.context.CopyResource(&self.staging, &src_tex);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&raw mut mapped))
                .map_err(|e| CaptureError::CaptureFailed(e.to_string()))?;
            let map_guard = StagingMapGuard {
                context: &self.context,
                staging: &self.staging,
            };

            let (row_bytes, pitch, total_bytes) = validate_mapped_frame_layout(
                self.width,
                self.height,
                mapped.RowPitch,
                mapped.pData.is_null(),
            )?;
            debug_assert!(pitch >= row_bytes);

            let mut data = vec![0u8; total_bytes];
            for y in 0..(self.height as usize) {
                let src_row =
                    std::slice::from_raw_parts(mapped.pData.cast::<u8>().add(y * pitch), row_bytes);
                let dst_start = y * row_bytes;
                data[dst_start..dst_start + row_bytes].copy_from_slice(src_row);
            }

            drop(map_guard);
            drop(frame_guard);
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

fn dirty_rect_from_bounds(left: i32, top: i32, right: i32, bottom: i32) -> Option<Rect> {
    let w = right.checked_sub(left)?;
    let h = bottom.checked_sub(top)?;
    if w <= 0 || h <= 0 {
        return None;
    }
    Some(Rect {
        x: left,
        y: top,
        width: u32::try_from(w).ok()?,
        height: u32::try_from(h).ok()?,
    })
}

fn validate_mapped_frame_layout(
    width: u32,
    height: u32,
    row_pitch: u32,
    pdata_is_null: bool,
) -> Result<(usize, usize, usize), CaptureError> {
    if pdata_is_null {
        return Err(CaptureError::CaptureFailed(
            "DXGI mapped staging texture has null pData".to_owned(),
        ));
    }
    if width == 0 || height == 0 {
        return Err(CaptureError::ScreenUnavailable(
            ScreenUnavailableReason::NoDisplay,
        ));
    }
    let width_usize = usize::try_from(width)
        .map_err(|_| CaptureError::CaptureFailed("DXGI frame width overflow".to_owned()))?;
    let height_usize = usize::try_from(height)
        .map_err(|_| CaptureError::CaptureFailed("DXGI frame height overflow".to_owned()))?;
    let row_bytes = width_usize
        .checked_mul(4)
        .ok_or_else(|| CaptureError::CaptureFailed("DXGI row_bytes overflow".to_owned()))?;
    let pitch = usize::try_from(row_pitch)
        .map_err(|_| CaptureError::CaptureFailed("DXGI RowPitch overflow".to_owned()))?;
    if pitch < row_bytes {
        return Err(CaptureError::CaptureFailed(format!(
            "DXGI RowPitch ({pitch}) is smaller than row_bytes ({row_bytes})"
        )));
    }
    let total_bytes = row_bytes
        .checked_mul(height_usize)
        .ok_or_else(|| CaptureError::CaptureFailed("DXGI pixel buffer size overflow".to_owned()))?;
    let _ = pitch
        .checked_mul(height_usize)
        .ok_or_else(|| CaptureError::CaptureFailed("DXGI mapped pitch span overflow".to_owned()))?;
    Ok((row_bytes, pitch, total_bytes))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_rect_uses_checked_sub_and_requires_positive_dimensions() {
        assert_eq!(
            dirty_rect_from_bounds(10, 20, 110, 220),
            Some(Rect {
                x: 10,
                y: 20,
                width: 100,
                height: 200,
            })
        );
        assert_eq!(dirty_rect_from_bounds(10, 20, 10, 220), None);
        assert_eq!(dirty_rect_from_bounds(10, 20, 110, 20), None);
        assert_eq!(dirty_rect_from_bounds(50, 50, 10, 100), None);
        assert_eq!(dirty_rect_from_bounds(i32::MIN, 0, i32::MAX, 10), None);
    }

    #[test]
    fn validate_mapped_layout_rejects_small_row_pitch_null_and_overflow() {
        assert!(matches!(
            validate_mapped_frame_layout(1920, 1080, 1920 * 4 - 4, false),
            Err(CaptureError::CaptureFailed(_))
        ));
        assert!(matches!(
            validate_mapped_frame_layout(1920, 1080, 1920 * 4, true),
            Err(CaptureError::CaptureFailed(_))
        ));
        assert!(matches!(
            validate_mapped_frame_layout(0, 1080, 64, false),
            Err(CaptureError::ScreenUnavailable(
                ScreenUnavailableReason::NoDisplay
            ))
        ));
        assert!(matches!(
            validate_mapped_frame_layout(u32::MAX, 4, u32::MAX, false),
            Err(CaptureError::CaptureFailed(_))
        ));

        let (row_bytes, pitch, total_bytes) =
            validate_mapped_frame_layout(1920, 1080, 1920 * 4 + 256, false)
                .expect("padded stride succeeds");
        assert_eq!(row_bytes, 1920 * 4);
        assert_eq!(pitch, 1920 * 4 + 256);
        assert_eq!(total_bytes, 1920 * 4 * 1080);
    }

    #[test]
    fn dxgi_locked_and_permission_missing_propagate_without_gdi_fallback() {
        for reason in [
            ScreenUnavailableReason::Locked,
            ScreenUnavailableReason::PermissionMissing,
            ScreenUnavailableReason::NoDisplay,
            ScreenUnavailableReason::Asleep,
        ] {
            let init_err: Result<((), DisplayGeometry), CaptureError> =
                Err(CaptureError::ScreenUnavailable(reason));
            let res = resolve_dxgi_init(init_err, || {
                panic!("GDI fallback must NOT mask ScreenUnavailable::{reason:?}")
            });
            assert!(matches!(res, Err(CaptureError::ScreenUnavailable(r)) if r == reason));

            let cap_res = route_dxgi_capture(Err(CaptureError::ScreenUnavailable(reason)), || {
                panic!("GDI capture must NOT mask ScreenUnavailable::{reason:?}")
            });
            assert!(matches!(cap_res, Err(CaptureError::ScreenUnavailable(r)) if r == reason));
        }

        // Non-availability driver errors still fall back to GDI cleanly.
        let mut fallback_called = false;
        let res = resolve_dxgi_init::<(), _>(
            Err(CaptureError::CaptureFailed(
                "DXGI unsupported surface".to_owned(),
            )),
            || {
                fallback_called = true;
                Ok(DisplayGeometry {
                    width: 1280,
                    height: 720,
                    refresh_hz: 60,
                    monitor_name: "GDI-Fallback".to_owned(),
                })
            },
        )
        .expect("GDI init fallback succeeds");
        assert!(fallback_called);
        assert!(res.0.is_none());
        assert_eq!(res.1.width, 1280);
    }

    #[test]
    fn probe_hardware_h264_mfts_releases_com_objects_cleanly() {
        let _ = probe_hardware_h264_mfts();
    }
}
