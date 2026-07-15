//! Linux hardware probing: RAM from `/proc/meminfo`, VRAM from `nvidia-smi`.

use std::fs;
use std::process::Command;

use auto_quantize_core::HardwareProfile;

/// Probe RAM and (if available) NVIDIA VRAM on a Linux host.
pub fn probe() -> HardwareProfile {
    let (ram_bytes, ram_free_bytes) = read_meminfo().unwrap_or((0, 0));
    let vram_bytes = probe_nvidia_vram();

    HardwareProfile {
        vram_bytes,
        ram_bytes,
        ram_free_bytes,
        // Effective memory bandwidth requires a micro-benchmark we don't yet
        // run; report unknown rather than guess (see docs/VISION.md).
        bandwidth_gbps: None,
    }
}

/// Reads and parses `/proc/meminfo`, returning `(MemTotal, MemAvailable)` in
/// bytes. Returns `None` if the file can't be read.
fn read_meminfo() -> Option<(u64, u64)> {
    let contents = fs::read_to_string("/proc/meminfo").ok()?;
    parse_meminfo(&contents)
}

/// Parses the contents of `/proc/meminfo` for `MemTotal` and `MemAvailable`,
/// both reported in kB, and returns them as bytes.
fn parse_meminfo(contents: &str) -> Option<(u64, u64)> {
    let mut total_kb = None;
    let mut available_kb = None;

    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total_kb = parse_kb_value(rest);
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available_kb = parse_kb_value(rest);
        }
    }

    let total_kb = total_kb?;
    // Older kernels may lack MemAvailable; fall back to total as a
    // conservative (over-)estimate of free memory.
    let available_kb = available_kb.unwrap_or(total_kb);

    Some((total_kb * 1024, available_kb * 1024))
}

/// Parses a `/proc/meminfo` value field like `"   16384000 kB"` into kB.
fn parse_kb_value(field: &str) -> Option<u64> {
    field.split_whitespace().next()?.parse().ok()
}

/// Queries `nvidia-smi` for total VRAM on the first GPU, in bytes.
/// Returns `None` if `nvidia-smi` isn't installed or reports no GPU.
fn probe_nvidia_vram() -> Option<u64> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_nvidia_smi_memory(&String::from_utf8_lossy(&output.stdout))
}

/// Parses `nvidia-smi --query-gpu=memory.total --format=csv,noheader,nounits`
/// output (one line per GPU, value in MiB) and returns the first GPU's
/// memory in bytes.
fn parse_nvidia_smi_memory(output: &str) -> Option<u64> {
    let first_line = output.lines().next()?.trim();
    let mib: u64 = first_line.parse().ok()?;
    Some(mib * 1024 * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meminfo_with_available() {
        let fixture = "\
MemTotal:       16336864 kB
MemFree:         1234567 kB
MemAvailable:    9876543 kB
Buffers:          123456 kB
";
        let (total, available) = parse_meminfo(fixture).unwrap();
        assert_eq!(total, 16_336_864 * 1024);
        assert_eq!(available, 9_876_543 * 1024);
    }

    #[test]
    fn parses_meminfo_without_available_falls_back_to_total() {
        let fixture = "MemTotal:       8000000 kB\nMemFree:        2000000 kB\n";
        let (total, available) = parse_meminfo(fixture).unwrap();
        assert_eq!(total, 8_000_000 * 1024);
        assert_eq!(available, 8_000_000 * 1024);
    }

    #[test]
    fn parses_meminfo_missing_total_returns_none() {
        assert!(parse_meminfo("MemFree: 123 kB\n").is_none());
    }

    #[test]
    fn parses_meminfo_empty_returns_none() {
        assert!(parse_meminfo("").is_none());
    }

    #[test]
    fn parses_nvidia_smi_single_gpu() {
        let output = "24576\n";
        assert_eq!(parse_nvidia_smi_memory(output), Some(24_576 * 1024 * 1024));
    }

    #[test]
    fn parses_nvidia_smi_uses_first_gpu_of_multiple() {
        let output = "24576\n8192\n";
        assert_eq!(parse_nvidia_smi_memory(output), Some(24_576 * 1024 * 1024));
    }

    #[test]
    fn parses_nvidia_smi_empty_output_returns_none() {
        assert!(parse_nvidia_smi_memory("").is_none());
    }

    #[test]
    fn parses_nvidia_smi_malformed_output_returns_none() {
        assert!(parse_nvidia_smi_memory("No devices were found\n").is_none());
    }
}
