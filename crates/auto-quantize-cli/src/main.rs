mod catalog;
mod download;
mod errors;
mod probe;

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use auto_quantize_core::QuantOption;
use clap::{Parser, Subcommand};
use errors::AppError;

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
    /// Recommend the best-fitting GGUF quant for a HuggingFace repo.
    Recommend {
        /// HuggingFace repo id, e.g. "TheBloke/Llama-2-7B-Chat-GGUF".
        repo: String,
        /// Emit a single machine-readable JSON object instead of text.
        #[arg(long)]
        json: bool,
        /// Skip the download confirmation prompt and download immediately.
        #[arg(short, long)]
        yes: bool,
        /// Print hardware-probe timing to stderr.
        #[arg(long)]
        timing: bool,
        /// Directory to download the recommended file(s) into.
        #[arg(short, long, default_value = ".")]
        output: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Command::Probe) | None => {
            let profile = probe::probe();
            println!("{:?}", profile);
            Ok(())
        }
        Some(Command::Recommend {
            repo,
            json,
            yes,
            timing,
            output,
        }) => run_recommend(&repo, json, yes, timing, &output),
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(err.exit_code());
    }
}

fn run_recommend(
    repo: &str,
    json: bool,
    yes: bool,
    timing: bool,
    output_dir: &std::path::Path,
) -> Result<(), AppError> {
    if !json {
        eprintln!("Probing hardware...");
    }
    let probe_start = Instant::now();
    let hardware = probe::probe();
    let probe_elapsed = probe_start.elapsed();
    if timing {
        eprintln!("  hardware probe took {:.3}s", probe_elapsed.as_secs_f64());
    }

    let catalog_quants = catalog::fetch_quants(repo)?;
    let options: Vec<QuantOption> = catalog_quants.iter().map(|c| c.option.clone()).collect();
    let recommendation = auto_quantize_core::recommend(&hardware, &options)
        .expect("catalog::fetch_quants never returns an empty quant list");

    if json {
        let payload = serde_json::json!({
            "hardware": hardware,
            "recommendation": {
                "quant": recommendation.quant.name,
                "size_bytes": recommendation.quant.size_bytes,
                "fits_fully": recommendation.fits_fully,
            },
            "reason": recommendation.reason,
        });
        println!("{payload}");
    } else {
        println!(
            "Recommendation: {} ({:.1} GB)",
            recommendation.quant.name,
            recommendation.quant.size_bytes as f64 / 1e9
        );
        println!("  {}", recommendation.reason);
    }

    // JSON mode is for scripting: never block on stdin. Downloading there
    // requires --yes explicitly.
    let should_download = yes || (!json && confirm_download());

    if should_download {
        let matched = catalog_quants
            .iter()
            .find(|c| c.option.name == recommendation.quant.name)
            .expect("recommendation always names a quant present in the catalog");
        download::download_files(repo, &matched.files, output_dir)?;
    }

    Ok(())
}

fn confirm_download() -> bool {
    print!("Download this build? [Y/n] ");
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    let answer = input.trim().to_lowercase();
    answer.is_empty() || answer == "y" || answer == "yes"
}
