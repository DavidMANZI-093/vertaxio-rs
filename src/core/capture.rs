use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::{
    Win32::Foundation::*, Win32::Graphics::Direct3D::*, Win32::Graphics::Direct3D11::*,
    Win32::Graphics::Dxgi::Common::*, Win32::Graphics::Dxgi::*, core::Interface,
};

use crate::services::errors::XError;
use crate::utils::logger;

pub struct DXGICapture {
    duplication: IDXGIOutputDuplication,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    pub texture_desc: D3D11_TEXTURE2D_DESC,
}

impl DXGICapture {
    pub fn new(hmonitor: HMONITOR) -> Result<Self, XError> {
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1()
                .map_err(|e| XError::SystemError(format!("CreateDXGIFactory1 failed: {}", e)))?;

            let mut adapter_idx = 0;
            let mut target_adapter = None;
            let mut target_output = None;

            while let Ok(adapter) = factory.EnumAdapters1(adapter_idx) {
                let mut output_idx = 0;
                while let Ok(output) = adapter.EnumOutputs(output_idx) {
                    if let Ok(desc) = output.GetDesc() {
                        if desc.Monitor == hmonitor {
                            target_adapter = Some(adapter.clone());
                            target_output = Some(output);
                            break;
                        }
                    }
                    output_idx += 1;
                }
                if target_output.is_some() {
                    break;
                }
                adapter_idx += 1;
            }

            let adapter = target_adapter.ok_or_else(|| {
                XError::SystemError("Failed to find DXGI Adapter for monitor".into())
            })?;
            let output = target_output.ok_or_else(|| {
                XError::SystemError("Failed to find DXGI Output for monitor".into())
            })?;

            let output1: IDXGIOutput1 = output
                .cast()
                .map_err(|e| XError::SystemError(format!("Failed to cast IDXGIOutput1: {}", e)))?;

            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;

            let feature_levels = [D3D_FEATURE_LEVEL_11_0];

            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&feature_levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
            .map_err(|e| XError::SystemError(format!("D3D11CreateDevice failed: {}", e)))?;

            let device = device.unwrap();
            let context = context.unwrap();

            let duplication = output1
                .DuplicateOutput(&device)
                .map_err(|e| XError::SystemError(format!("DuplicateOutput failed: {}", e)))?;

            let out_desc = duplication.GetDesc();

            let texture_desc = D3D11_TEXTURE2D_DESC {
                Width: out_desc.ModeDesc.Width,
                Height: out_desc.ModeDesc.Height,
                MipLevels: 1,
                ArraySize: 1,
                Format: out_desc.ModeDesc.Format,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };

            logger::info(&format!(
                "DXGI Capture initialized for {}x{}",
                texture_desc.Width, texture_desc.Height
            ));

            Ok(Self {
                duplication,
                device,
                context,
                texture_desc,
            })
        }
    }

    pub fn grab_frame(&mut self, timeout_ms: u32) -> Result<Vec<u8>, XError> {
        unsafe {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut resource: Option<IDXGIResource> = None;

            match self
                .duplication
                .AcquireNextFrame(timeout_ms, &mut frame_info, &mut resource)
            {
                Ok(_) => {}
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {
                    return Err(XError::Timeout);
                }
                Err(e) => {
                    return Err(XError::SystemError(format!(
                        "AcquireNextFrame failed: {}",
                        e
                    )));
                }
            }

            let resource = resource.unwrap();
            let texture2d: ID3D11Texture2D = resource.cast().map_err(|e| {
                let _ = self.duplication.ReleaseFrame();
                XError::SystemError(format!("Failed to cast resource to Texture2D: {}", e))
            })?;

            let mut staging_texture: Option<ID3D11Texture2D> = None;
            self.device
                .CreateTexture2D(&self.texture_desc, None, Some(&mut staging_texture))
                .map_err(|e| {
                    let _ = self.duplication.ReleaseFrame();
                    XError::SystemError(format!("CreateTexture2D failed: {}", e))
                })?;
            let staging_texture = staging_texture.unwrap();

            self.context.CopyResource(&staging_texture, &texture2d);

            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context
                .Map(&staging_texture, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|e| {
                    let _ = self.duplication.ReleaseFrame();
                    XError::SystemError(format!("Map failed: {}", e))
                })?;

            let bytes_per_pixel = 4; // BGRA
            let row_pitch = mapped.RowPitch as usize;
            let width_bytes = (self.texture_desc.Width * bytes_per_pixel) as usize;

            let mut buffer = vec![
                0u8;
                (self.texture_desc.Width * self.texture_desc.Height * bytes_per_pixel)
                    as usize
            ];

            let src_ptr = mapped.pData as *const u8;
            for y in 0..self.texture_desc.Height as usize {
                let src_row = src_ptr.add(y * row_pitch);
                let dst_row = buffer.as_mut_ptr().add(y * width_bytes);
                std::ptr::copy_nonoverlapping(src_row, dst_row, width_bytes);
            }

            self.context.Unmap(&staging_texture, 0);
            let _ = self.duplication.ReleaseFrame();

            Ok(buffer)
        }
    }
}
