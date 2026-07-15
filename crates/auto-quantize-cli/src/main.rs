mod catalog;
mod download;
mod errors;
mod probe;

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use auto_quantize_core::{ContextConfig, Preference, QuantOption};
use clap::{Parser, Subcommand, ValueEnum};
use errors::AppError;

/// CLI-facing mirror of [`Preference`] so clap can parse it from `--prefer`.
#[derive(Clone, Copy, ValueEnum)]
enum PreferArg {
    Quality,
    Speed,
}

impl From<PreferArg> for Preference {
    fn from(arg: PreferArg) -> Self {
        match arg {
            PreferArg::Quality => Preference::Quality,
            PreferArg::Speed => Preference::Speed,
        }
    }
}

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
        /// Reserve this many extra GB of accelerator budget beyond the
        /// default headroom, shifting the recommendation toward smaller quants.
        #[arg(long, default_value_t = 0.0)]
        reserve_vram: f64,
        /// Break ties between similarly-fitting quants toward quality
        /// (larger, default) or speed (smaller, more headroom).
        #[arg(long, value_enum, default_value = "quality")]
        prefer: PreferArg,
        /// Assumed context length (in tokens) for KV-cache headroom sizing.
        /// Requires the repo's config.json to expose a recognized layer
        /// count/hidden size; falls back to the default headroom otherwise.
        #[arg(long)]
        context: Option<u32>,
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
            reserve_vram,
            prefer,
            context,
        }) => run_recommend(
            &repo,
            json,
            yes,
            timing,
            &output,
            reserve_vram,
            prefer.into(),
            context,
        ),
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(err.exit_code());
    }
}

#[allow(clippy::too_many_arguments)]
fn run_recommend(
    repo: &str,
    json: bool,
    yes: bool,
    timing: bool,
    output_dir: &std::path::Path,
    reserve_vram_gb: f64,
    prefer: Preference,
    context_length: Option<u32>,
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
    let reserve_bytes = (reserve_vram_gb.max(0.0) * 1e9) as u64;

    let context =
        context_length.and_then(|context_length| match catalog::fetch_architecture(repo) {
            Some(architecture) => Some(ContextConfig {
                context_length,
                architecture,
            }),
            None => {
                if !json {
                    eprintln!(
                        "  could not determine model architecture for '{repo}'; \
                         ignoring --context and using the default headroom"
                    );
                }
                None
            }
        });

    let recommendation = auto_quantize_core::recommend_with_context(
        &hardware,
        &options,
        reserve_bytes,
        prefer,
        context,
    )
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
    confirm(std::io::stdin().lock())
}

/// Parses a yes/no answer from `reader`. A real Enter keypress sends an
/// empty *line* (just `"\n"`) and defaults to yes, per the `[Y/n]` prompt;
/// EOF with zero bytes read (closed/non-interactive stdin, e.g. piped from
/// `/dev/null` or a CI runner without a tty) is not the same thing and must
/// default to no, so a script that forgets `--yes` can't silently trigger a
/// multi-gigabyte download.
fn confirm(mut reader: impl std::io::BufRead) -> bool {
    let mut input = String::new();
    let bytes_read = match reader.read_line(&mut input) {
        Ok(n) => n,
        Err(_) => return false,
    };
    if bytes_read == 0 {
        return false;
    }
    let answer = input.trim().to_lowercase();
    answer.is_empty() || answer == "y" || answer == "yes"
}

#[cfg(test)]
mod confirm_tests {
    use super::confirm;

    #[test]
    fn real_enter_keypress_defaults_to_yes() {
        assert!(confirm(b"\n".as_slice()));
    }

    #[test]
    fn eof_with_no_input_at_all_defaults_to_no() {
        assert!(!confirm(b"".as_slice()));
    }

    #[test]
    fn explicit_y_confirms() {
        assert!(confirm(b"y\n".as_slice()));
    }

    #[test]
    fn explicit_yes_confirms() {
        assert!(confirm(b"Yes\n".as_slice()));
    }

    #[test]
    fn explicit_n_declines() {
        assert!(!confirm(b"n\n".as_slice()));
    }

    #[test]
    fn garbage_input_declines() {
        assert!(!confirm(b"asdf\n".as_slice()));
    }
}
