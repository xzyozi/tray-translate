//! Issue #5: Argos の言語パッケージを ct2rs で動かし、Python(Argos)と出力を比較する確認用。
//!
//! 使い方:
//!   translate --package <argos パッケージのディレクトリ> --input <sentences.json> \
//!             --from en --output rust_en_ja.json [--repeat 5]
//!
//! パッケージのディレクトリには `model/`(CTranslate2 モデル)と `sentencepiece.model` がある。
//! 文分割は行わない(テスト文は1文ずつのため、Argos の文分割結果と同じ単位になる)。

use std::{env, fs, path::PathBuf, process::ExitCode, time::Instant};

use ct2rs::{tokenizers::sentencepiece::Tokenizer, Config, TranslationOptions, Translator};
use serde_json::{json, Value};

struct Args {
    package: PathBuf,
    input: PathBuf,
    from: String,
    output: PathBuf,
    repeat: usize,
}

fn parse_args() -> Result<Args, String> {
    let mut package = None;
    let mut input = None;
    let mut from = None;
    let mut output = None;
    let mut repeat = 5_usize;

    let mut iter = env::args().skip(1);
    while let Some(flag) = iter.next() {
        let value = iter.next().ok_or(format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--package" => package = Some(PathBuf::from(value)),
            "--input" => input = Some(PathBuf::from(value)),
            "--from" => from = Some(value),
            "--output" => output = Some(PathBuf::from(value)),
            "--repeat" => repeat = value.parse().map_err(|_| "invalid --repeat".to_string())?,
            _ => return Err(format!("unknown option: {flag}")),
        }
    }

    Ok(Args {
        package: package.ok_or("--package is required")?,
        input: input.ok_or("--input is required")?,
        from: from.ok_or("--from is required")?,
        output: output.ok_or("--output is required")?,
        repeat,
    })
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.total_cmp(b));
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

fn run(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let doc: Value = serde_json::from_str(&fs::read_to_string(&args.input)?)?;
    let sentences = doc["sentences"].as_array().ok_or("sentences not found")?;

    // Argos と同じ翻訳オプション(translate.py の translate_batch 呼び出しに合わせる)
    let options: TranslationOptions<String, String> = TranslationOptions {
        beam_size: 4,
        length_penalty: 0.2,
        replace_unknowns: true,
        ..Default::default()
    };

    let spm = args.package.join("sentencepiece.model");
    let load_start = Instant::now();
    let translator = Translator::with_tokenizer(
        args.package.join("model"),
        Tokenizer::from_file(&spm, &spm)?,
        &Config::default(),
    )?;
    let load_sec = load_start.elapsed().as_secs_f64();

    let translate = |text: &str| -> Result<String, Box<dyn std::error::Error>> {
        let results = translator.translate_batch(&[text], &options, None)?;
        Ok(results.into_iter().next().map(|(t, _)| t).unwrap_or_default())
    };

    let source_of = |s: &Value| -> Result<String, Box<dyn std::error::Error>> {
        Ok(s[args.from.as_str()].as_str().ok_or("source text not found")?.to_string())
    };

    // 初回呼び出しの時間を別に計測する(Python 版の cold start に対応)
    let first = source_of(&sentences[0])?;
    let start = Instant::now();
    translate(&first)?;
    let cold_start_sec = start.elapsed().as_secs_f64();

    // 直前と同じ入力が続かないよう、最後の文でならしてから全文を周回する(Python 版と同じ方式)
    translate(&source_of(&sentences[sentences.len() - 1])?)?;

    let mut timings: Vec<Vec<f64>> = vec![Vec::new(); sentences.len()];
    let mut outputs: Vec<String> = vec![String::new(); sentences.len()];
    for _ in 0..args.repeat {
        for (i, s) in sentences.iter().enumerate() {
            let text = source_of(s)?;
            let start = Instant::now();
            outputs[i] = translate(&text)?;
            timings[i].push(start.elapsed().as_secs_f64());
        }
    }

    let medians: Vec<f64> = timings.iter_mut().map(|t| median(t)).collect();
    let results: Vec<Value> = sentences
        .iter()
        .enumerate()
        .map(|(i, s)| {
            json!({
                "id": s["id"],
                "category": s["category"],
                "source": s[args.from.as_str()],
                "output": outputs[i],
                "median_sec": medians[i],
            })
        })
        .collect();

    let summary_median = median(&mut medians.clone());
    let report = json!({
        "engine": "ct2rs",
        "repeat": args.repeat,
        "summary": { "load_sec": load_sec, "cold_start_sec": cold_start_sec, "median_sec": summary_median },
        "results": results,
    });
    fs::write(&args.output, serde_json::to_string_pretty(&report)? + "\n")?;

    println!("load={load_sec:.2}s cold_start={cold_start_sec:.2}s median={summary_median:.3}s");
    println!("saved: {}", args.output.display());
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
