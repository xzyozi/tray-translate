"""Argos Translate の基準値計測スクリプト(Issue #4)。

CPU のみで、速度・メモリ・翻訳結果を計測して JSON に保存する。
実行例(Linux、uv 使用):

    uv run --extra bench python bench/argos_baseline.py --from en --to ja --install
    uv run --extra bench python bench/argos_baseline.py --from ja --to en
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import statistics
import sys
import time
from importlib import metadata
from pathlib import Path
from typing import Any

# CPU のみで計測する。argostranslate の import より前に設定する必要がある
os.environ["ARGOS_DEVICE_TYPE"] = "cpu"

DEFAULT_INPUT = Path(__file__).resolve().parent.parent / "docs" / "testdata" / "sentences.json"


def log(message: str) -> None:
    """進捗を標準エラーに即時出力する(長い処理で止まって見えないようにする)。"""
    print(f"[{time.strftime('%H:%M:%S')}] {message}", file=sys.stderr, flush=True)


def peak_rss_mb() -> float | None:
    """プロセスのピークメモリ(MB)を返す。Linux 以外では取得できないため None。"""
    try:
        import resource
    except ImportError:
        return None
    # Linux は KB 単位
    return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024


def package_version(name: str) -> str | None:
    try:
        return metadata.version(name)
    except metadata.PackageNotFoundError:
        return None


def install_package(from_code: str, to_code: str) -> None:
    """言語パッケージを索引から取得してインストールする(ネットワークが必要)。"""
    log("importing argostranslate.package (初回は数十秒かかることがある)")
    import argostranslate.package

    log("updating package index (ネットワーク通信)")
    argostranslate.package.update_package_index()
    available = argostranslate.package.get_available_packages()
    package = next((p for p in available if p.from_code == from_code and p.to_code == to_code), None)
    if package is None:
        raise SystemExit(f"package not found: {from_code} -> {to_code}")
    log(f"downloading package {from_code}->{to_code} (数百MBのことがある)")
    path = package.download()
    log(f"installing package from {path}")
    argostranslate.package.install_from_path(path)
    log("package installed")


def percentile(values: list[float], ratio: float) -> float:
    ordered = sorted(values)
    index = min(len(ordered) - 1, int(len(ordered) * ratio))
    return ordered[index]


def run(args: argparse.Namespace) -> dict[str, Any]:
    sentences: list[dict[str, str]] = json.loads(args.input.read_text(encoding="utf-8"))["sentences"]

    if args.install:
        install_package(args.from_code, args.to_code)

    log("importing argostranslate.translate")
    import argostranslate.translate

    if argostranslate.translate.get_translation_from_codes(args.from_code, args.to_code) is None:
        raise SystemExit(
            f"language package not installed: {args.from_code} -> {args.to_code}. "
            "--install を付けて実行してください。"
        )

    # 初回呼び出しにはモデルの読み込みが含まれるため、別に計測する
    log("cold start translate (モデル読み込みを含む)")
    first = sentences[0][args.from_code]
    start = time.perf_counter()
    argostranslate.translate.translate(first, args.from_code, args.to_code)
    cold_start_sec = time.perf_counter() - start
    log(f"cold start done: {cold_start_sec:.2f}s")

    # Argos は直前の翻訳結果を段落単位でキャッシュする(CachedTranslation)。
    # 同じ入力を連続して呼ぶとキャッシュに当たり計測が無意味になるため、
    # 全文を順に翻訳する周回を繰り返し、直前と異なる入力が常に続くようにする。
    # 周回の先頭が cold start の入力と連続しないよう、最後の文で一度ならしておく(計測しない)。
    argostranslate.translate.translate(sentences[-1][args.from_code], args.from_code, args.to_code)

    timings: dict[str, list[float]] = {s["id"]: [] for s in sentences}
    outputs: dict[str, str] = {}
    for round_no in range(args.repeat):
        log(f"round {round_no + 1}/{args.repeat}")
        for sentence in sentences:
            start = time.perf_counter()
            outputs[sentence["id"]] = argostranslate.translate.translate(
                sentence[args.from_code], args.from_code, args.to_code
            )
            timings[sentence["id"]].append(time.perf_counter() - start)

    results: list[dict[str, Any]] = [
        {
            "id": sentence["id"],
            "category": sentence["category"],
            "source": sentence[args.from_code],
            "reference": sentence.get(args.to_code),
            "output": outputs[sentence["id"]],
            "chars": len(sentence[args.from_code]),
            "median_sec": statistics.median(timings[sentence["id"]]),
        }
        for sentence in sentences
    ]

    medians = [r["median_sec"] for r in results]
    total_chars = sum(r["chars"] for r in results)
    by_category: dict[str, dict[str, float]] = {}
    for category in sorted({r["category"] for r in results}):
        group = [r for r in results if r["category"] == category]
        by_category[category] = {
            "median_sec": statistics.median(r["median_sec"] for r in group),
            "chars_per_sec": sum(r["chars"] for r in group) / sum(r["median_sec"] for r in group),
        }
    return {
        "environment": {
            "os": platform.platform(),
            "machine": platform.machine(),
            "cpu_count": os.cpu_count(),
            "python": platform.python_version(),
            "argostranslate": package_version("argostranslate"),
            "ctranslate2": package_version("ctranslate2"),
            "device": os.environ["ARGOS_DEVICE_TYPE"],
        },
        "pair": f"{args.from_code}->{args.to_code}",
        "repeat": args.repeat,
        "summary": {
            "cold_start_sec": cold_start_sec,
            "median_sec": statistics.median(medians),
            "p95_sec": percentile(medians, 0.95),
            "chars_per_sec": total_chars / sum(medians),
            "peak_rss_mb": peak_rss_mb(),
            "by_category": by_category,
        },
        "results": results,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--from", dest="from_code", required=True, help="翻訳元の言語コード(例: en)")
    parser.add_argument("--to", dest="to_code", required=True, help="翻訳先の言語コード(例: ja)")
    parser.add_argument("--input", type=Path, default=DEFAULT_INPUT, help="テスト文セットの JSON")
    parser.add_argument("--output", type=Path, default=None, help="結果の保存先(既定: bench_<from>_<to>.json)")
    parser.add_argument("--repeat", type=int, default=5, help="全文を翻訳する周回の回数")
    parser.add_argument("--install", action="store_true", help="言語パッケージを取得してインストールする")
    args = parser.parse_args()

    output = args.output or Path(f"bench_{args.from_code}_{args.to_code}.json")
    report = run(args)
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")

    summary = report["summary"]
    print(f"pair={report['pair']} cold_start={summary['cold_start_sec']:.2f}s median={summary['median_sec']:.3f}s")
    print(f"p95={summary['p95_sec']:.3f}s chars/s={summary['chars_per_sec']:.1f} peak_rss_mb={summary['peak_rss_mb']}")
    for category, values in summary["by_category"].items():
        print(f"  {category}: median={values['median_sec']:.3f}s chars/s={values['chars_per_sec']:.1f}")
    print(f"cpu_count={report['environment']['cpu_count']} saved: {output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
