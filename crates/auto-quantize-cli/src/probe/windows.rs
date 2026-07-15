//! Windows hardware probing (docs/BACKLOG.md 1.4): RAM via
//! `GlobalMemoryStatusEx`, VRAM via DXGI adapter enumeration.

use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

use auto_quantize_core::HardwareProfile;

/// PCI vendor ID Windows always exposes for its software-rasterizer
/// fallback adapter ("Microsoft Basic Render Driver"), which every DXGI
/// enumeration includes alongside any real GPU. Not a machine to recommend
/// quants against, so it's skipped.
const MICROSOFT_BASIC_RENDER_VENDOR_ID: u32 = 0x1414;

/// Probe RAM and (if available) VRAM on a Windows host.
pub fn probe() -> HardwareProfile {
    let (ram_bytes, ram_free_bytes) = probe_memory().unwrap_or((0, 0));
    let vram_bytes = probe_vram();

    HardwareProfile {
        vram_bytes,
        ram_bytes,
        ram_free_bytes,
        // Effective memory bandwidth requires a micro-benchmark we don't yet
        // run; report unknown rather than guess (see docs/VISION.md).
        bandwidth_gbps: None,
    }
}

/// Queries total and available physical memory via `GlobalMemoryStatusEx`.
fn probe_memory() -> Option<(u64, u64)> {
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;
    Some((status.ullTotalPhys, status.ullAvailPhys))
}

/// Enumerates DXGI adapters and returns the dedicated VRAM of the first one
/// that isn't the Microsoft Basic Render software fallback. Returns `None`
/// if DXGI is unavailable or every adapter is a software rasterizer, rather
/// than guessing.
fn probe_vram() -> Option<u64> {
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        for index in 0u32.. {
            let adapter = match factory.EnumAdapters1(index) {
                Ok(adapter) => adapter,
                // DXGI_ERROR_NOT_FOUND once `index` exceeds the adapter count.
                Err(_) => break,
            };
            let desc = adapter.GetDesc1().ok()?;
            if is_software_adapter(desc.VendorId) {
                continue;
            }
            return Some(desc.DedicatedVideoMemory as u64);
        }
        None
    }
}

/// Whether `vendor_id` identifies Windows' software-rasterizer fallback
/// adapter rather than a real GPU.
fn is_software_adapter(vendor_id: u32) -> bool {
    vendor_id == MICROSOFT_BASIC_RENDER_VENDOR_ID
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_microsoft_basic_render_vendor_id() {
        assert!(is_software_adapter(0x1414));
    }

    #[test]
    fn does_not_flag_a_real_gpu_vendor_id() {
        // NVIDIA
        assert!(!is_software_adapter(0x10DE));
        // AMD
        assert!(!is_software_adapter(0x1002));
        // Intel
        assert!(!is_software_adapter(0x8086));
    }
}
