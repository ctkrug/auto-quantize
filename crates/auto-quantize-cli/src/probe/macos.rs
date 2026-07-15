//! macOS hardware probing (docs/BACKLOG.md 1.3): RAM via `sysctl`/`vm_stat`,
//! VRAM via the unified-memory assumption on Apple Silicon or
//! `system_profiler` on Intel Macs with a discrete GPU.

use std::process::Command;

use auto_quantize_core::HardwareProfile;

/// Probe RAM (and, where determinable, VRAM) on a macOS host.
pub fn probe() -> HardwareProfile {
    let ram_bytes = read_memsize().unwrap_or(0);
    let ram_free_bytes = read_vm_stat_free_bytes().unwrap_or(ram_bytes);

    // Apple Silicon shares one pool of unified memory between CPU and GPU,
    // so there's no separate VRAM figure to query: the whole `hw.memsize`
    // budget is available to the GPU, modulo whatever the OS itself is
    // using (already reflected by `ram_free_bytes`). Intel Macs instead
    // have a real discrete GPU with its own VRAM, queried separately.
    let vram_bytes = if cfg!(target_arch = "aarch64") {
        Some(ram_bytes)
    } else {
        read_system_profiler_vram()
    };

    HardwareProfile {
        vram_bytes,
        ram_bytes,
        ram_free_bytes,
        // Effective memory bandwidth requires a micro-benchmark we don't yet
        // run; report unknown rather than guess (see docs/VISION.md).
        bandwidth_gbps: None,
    }
}

fn read_memsize() -> Option<u64> {
    let output = Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_sysctl_memsize(&String::from_utf8_lossy(&output.stdout))
}

/// Parses `sysctl -n hw.memsize` output: a single integer, in bytes.
fn parse_sysctl_memsize(output: &str) -> Option<u64> {
    output.trim().parse().ok()
}

fn read_vm_stat_free_bytes() -> Option<u64> {
    let output = Command::new("vm_stat").output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_vm_stat_free_bytes(&String::from_utf8_lossy(&output.stdout))
}

/// Parses `vm_stat` output for free memory, in bytes: `Pages free:` times
/// the page size reported in the header (`Mach Virtual Memory Statistics:
/// (page size of N bytes)`), defaulting to 4096 if the header is missing.
fn parse_vm_stat_free_bytes(output: &str) -> Option<u64> {
    let page_size = output
        .lines()
        .next()
        .and_then(|header| header.split("page size of ").nth(1))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse::<u64>().ok())
        .unwrap_or(4096);

    let free_pages: u64 = output.lines().find_map(|line| {
        line.strip_prefix("Pages free:")
            .and_then(|rest| rest.trim().trim_end_matches('.').parse().ok())
    })?;

    Some(free_pages * page_size)
}

fn read_system_profiler_vram() -> Option<u64> {
    let output = Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_system_profiler_vram(&String::from_utf8_lossy(&output.stdout))
}

/// Parses the first `VRAM (...): <N> <unit>` line out of
/// `system_profiler SPDisplaysDataType` output, converting to bytes.
/// Returns `None` if no such line is found (e.g. no discrete GPU) rather
/// than guessing — an honest "unsupported" fallback, per docs/VISION.md.
fn parse_system_profiler_vram(output: &str) -> Option<u64> {
    let line = output.lines().find(|line| line.contains("VRAM"))?;
    let value_part = line.split(':').nth(1)?.trim();
    let mut parts = value_part.split_whitespace();
    let amount: u64 = parts.next()?.parse().ok()?;
    let unit = parts.next()?;

    let multiplier: u64 = match unit.to_ascii_uppercase().as_str() {
        "GB" => 1024 * 1024 * 1024,
        "MB" => 1024 * 1024,
        "KB" => 1024,
        _ => return None,
    };
    Some(amount * multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sysctl_memsize() {
        assert_eq!(parse_sysctl_memsize("17179869184\n"), Some(17_179_869_184));
    }

    #[test]
    fn parses_sysctl_memsize_empty_returns_none() {
        assert!(parse_sysctl_memsize("").is_none());
    }

    #[test]
    fn parses_sysctl_memsize_malformed_returns_none() {
        assert!(parse_sysctl_memsize("not a number\n").is_none());
    }

    #[test]
    fn parses_vm_stat_free_bytes_with_16k_pages() {
        let fixture = "\
Mach Virtual Memory Statistics: (page size of 16384 bytes)
Pages free:                               61391.
Pages active:                            237452.
Pages wired down:                        223648.
";
        assert_eq!(parse_vm_stat_free_bytes(fixture), Some(61_391 * 16_384));
    }

    #[test]
    fn parses_vm_stat_free_bytes_defaults_to_4k_pages_without_header() {
        let fixture = "Pages free:                               1000.\n";
        assert_eq!(parse_vm_stat_free_bytes(fixture), Some(1_000 * 4096));
    }

    #[test]
    fn parses_vm_stat_free_bytes_missing_field_returns_none() {
        let fixture =
            "Mach Virtual Memory Statistics: (page size of 4096 bytes)\nPages active: 100.\n";
        assert!(parse_vm_stat_free_bytes(fixture).is_none());
    }

    #[test]
    fn parses_vm_stat_free_bytes_empty_returns_none() {
        assert!(parse_vm_stat_free_bytes("").is_none());
    }

    #[test]
    fn parses_system_profiler_vram_in_gb() {
        let fixture = "\
Graphics/Displays:

    AMD Radeon Pro 5500M:

      Chipset Model: AMD Radeon Pro 5500M
      VRAM (Dynamic, Max): 8 GB
";
        assert_eq!(
            parse_system_profiler_vram(fixture),
            Some(8 * 1024 * 1024 * 1024)
        );
    }

    #[test]
    fn parses_system_profiler_vram_in_mb() {
        let fixture = "      VRAM (Total): 1536 MB\n";
        assert_eq!(
            parse_system_profiler_vram(fixture),
            Some(1536 * 1024 * 1024)
        );
    }

    #[test]
    fn parses_system_profiler_vram_missing_returns_none() {
        let fixture = "Graphics/Displays:\n\n    Apple M2:\n\n      Chipset Model: Apple M2\n";
        assert!(parse_system_profiler_vram(fixture).is_none());
    }

    #[test]
    fn parses_system_profiler_vram_empty_returns_none() {
        assert!(parse_system_profiler_vram("").is_none());
    }
}
