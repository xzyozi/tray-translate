use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use tray_translate_core::{get_model_dir, ModelHub, ModelPackage, TranslatorPipeline};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "translate" => {
            if args.len() < 4 {
                eprintln!("Usage: tt translate <package-dir-or-pair> <text>");
                eprintln!("Example: tt translate en_ja \"Hello world\"");
                std::process::exit(1);
            }
            let target = &args[2];
            let text = &args[3..].join(" ");

            // ディレクトリパスか言語ペアキー（例: "en_ja"）かを判定
            let package_dir = if PathBuf::from(target).exists() {
                PathBuf::from(target)
            } else {
                let model_dir = get_model_dir()?;
                let candidate = model_dir.join(target);
                if candidate.exists() {
                    candidate
                } else {
                    anyhow::bail!(
                        "Model package not found at '{}' or '{}'. Run 'tt download' first.",
                        target,
                        candidate.display()
                    );
                }
            };

            let package = ModelPackage::open(&package_dir)
                .with_context(|| format!("Failed to open package at {}", package_dir.display()))?;

            let pipeline = TranslatorPipeline::new(&package)?;
            let output = pipeline.translate(text)?;
            println!("{output}");
        }
        "download" => {
            if args.len() < 4 {
                eprintln!("Usage: tt download <from_code> <to_code>");
                eprintln!("Example: tt download en ja");
                std::process::exit(1);
            }
            let from = &args[2];
            let to = &args[3];

            println!("Fetching package index...");
            let index = ModelHub::fetch_index()?;
            let entry = ModelHub::find_package(&index, from, to)?;

            let model_dir = get_model_dir()?;
            println!(
                "Downloading {} -> {} (v{})...",
                entry.from_name, entry.to_name, entry.package_version
            );

            let package = ModelHub::download_and_install(&entry, &model_dir)?;
            println!(
                "Successfully installed package to: {}",
                package.root_dir().display()
            );
        }
        "list" => {
            println!("Fetching package index...");
            let index = ModelHub::fetch_index()?;
            println!("Available language packages ({}):", index.len());
            for p in &index {
                println!(
                    "  {: <4} -> {: <4} ({: <15} -> {: <15}) [v{}]",
                    p.from_code, p.to_code, p.from_name, p.to_name, p.package_version
                );
            }
        }
        "--help" | "-h" | "help" => {
            print_usage();
        }
        other => {
            eprintln!("Unknown command: {other}");
            print_usage();
            std::process::exit(1);
        }
    }

    Ok(())
}

fn print_usage() {
    println!(
        "tray-translate CLI (tt)\n\n\
        USAGE:\n\
            tt list                                   List available model packages\n\
            tt download <from> <to>                   Download & install model package\n\
            tt translate <package-or-pair> <text>     Translate text (e.g. tt translate en_ja \"Hello\")\n\
            tt help                                   Show this help message\n"
    );
}
