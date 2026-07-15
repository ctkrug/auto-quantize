mod probe;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "auto-quantize",
    version,
    about = "Benchmark your machine and recommend the best-fitting quantized model build"
)]
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
            let profile = probe::probe();
            println!("{:?}", profile);
        }
    }
}
