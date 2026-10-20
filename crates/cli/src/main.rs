use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use tray_translate_core::{ModelPackage, TranslatorPipeline};

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
                eprintln!("Usage: tt translate <package-dir> <text>");
                std::process::exit(1);
            }
            let package_dir = PathBuf::from(&args[2]);
            let text = &args[3..].join(" ");

            let package = ModelPackage::open(&package_dir)
                .with_context(|| format!("Failed to open package at {}", package_dir.display()))?;

            let pipeline = TranslatorPipeline::new(&package)?;
            let output = pipeline.translate(text)?;
            println!("{output}");
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
            tt translate <package-dir> <text>    Translate text using specified package\n\
            tt help                              Show this help message\n"
    );
}
