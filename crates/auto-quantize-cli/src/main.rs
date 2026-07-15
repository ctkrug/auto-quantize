use auto_quantize_core::HardwareProfile;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "auto-quantize", version, about = "Benchmark your machine and recommend the best-fitting quantized model build")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Print the detected hardware profile and exit.
    Probe,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Probe) | None => {
            // Real cross-platform probing lands in the BUILD phase (see
            // docs/BACKLOG.md, epic 1). This placeholder proves the CLI ->
            // core wiring works end to end.
            let profile = HardwareProfile {
                vram_bytes: None,
                ram_bytes: 0,
                ram_free_bytes: 0,
                bandwidth_gbps: None,
            };
            println!("auto-quantize: hardware probing not yet implemented");
            println!("{:?}", profile);
        }
    }
}
