use anyhow::{Context, Result};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;
use tray_translate_core::{
    get_model_dir, ModelHub, ModelPackage, TranslationRouter, TranslatorPipeline,
};

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
                eprintln!("Usage: tt translate <pair-or-dir> <text>");
                eprintln!("Example: tt translate en_ja \"Hello world\"");
                std::process::exit(1);
            }
            let target = &args[2];
            let text = &args[3..].join(" ");

            // target が "ja->es" や "en_ja" のような言語ペアか、ディレクトリパスかを判定
            if target.contains('_') || target.contains("->") {
                let parts: Vec<&str> = if target.contains("->") {
                    target.split("->").collect()
                } else {
                    target.split('_').collect()
                };

                if parts.len() == 2 {
                    let from = parts[0].trim();
                    let to = parts[1].trim();

                    let mut router = TranslationRouter::new()?;
                    let output = router.translate(from, to, text)?;
                    println!("{output}");
                    return Ok(());
                }
            }

            // ディレクトリパス指定の場合
            let package_dir = PathBuf::from(target);
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
        "bench" => {
            handle_bench(&args[2..])?;
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

fn handle_bench(args: &[String]) -> Result<()> {
    if args.is_empty() {
        eprintln!(
            "Usage: tt bench <package-dir-or-pair> <sentences.json> [--compare <py_bench.json>]"
        );
        std::process::exit(1);
    }

    let target = &args[0];
    let sentences_path = Path::new(&args[1]);
    let compare_path = if args.len() >= 4 && args[2] == "--compare" {
        Some(Path::new(&args[3]))
    } else {
        None
    };

    let doc: Value = serde_json::from_str(&fs::read_to_string(sentences_path)?)?;
    let sentences = doc["sentences"].as_array().context("sentences not found")?;

    let parts: Vec<&str> = if target.contains("->") {
        target.split("->").collect()
    } else {
        target.split('_').collect()
    };
    let from_key = parts[0].trim();

    let mut router = TranslationRouter::new()?;
    let to_key = parts[1].trim();

    println!("Running benchmark on {} sentences...", sentences.len());

    let mut rust_results = Vec::new();
    for s in sentences {
        let id = s["id"].as_str().unwrap_or_default();
        let input_text = s[from_key].as_str().context("source text not found")?;
        let output = router.translate(from_key, to_key, input_text)?;
        rust_results.push((id.to_string(), input_text.to_string(), output));
    }

    if let Some(comp_file) = compare_path {
        let py_doc: Value = serde_json::from_str(&fs::read_to_string(comp_file)?)?;
        let py_results = py_doc["results"]
            .as_array()
            .context("py results not found")?;

        let mut py_map = std::collections::HashMap::new();
        for r in py_results {
            if let (Some(id), Some(out)) = (r["id"].as_str(), r["output"].as_str()) {
                py_map.insert(id, out);
            }
        }

        let mut match_count = 0;
        println!("\n=== G3 一致比較結果 (Rust vs Python) ===");
        for (id, _input, rs_out) in &rust_results {
            if let Some(py_out) = py_map.get(id.as_str()) {
                let is_match = rs_out.trim() == py_out.trim();
                if is_match {
                    match_count += 1;
                    println!("[MATCH] {id}");
                } else {
                    println!("[DIFF ] {id}");
                    println!("   Py: {py_out}");
                    println!("   Rs: {rs_out}");
                }
            }
        }

        let total = rust_results.len();
        let pct = (match_count as f64 / total as f64) * 100.0;
        println!("\n一致率: {match_count} / {total} ({pct:.1}%)");
    } else {
        for (id, _input, output) in &rust_results {
            println!("[{id}] {output}");
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
            tt translate <pair-or-dir> <text>         Translate text (e.g. tt translate en_ja \"Hello\")\n\
            tt bench <pair> <sentences.json>          Run benchmark and compare with Python (G3)\n\
            tt help                                   Show this help message\n"
    );
}
