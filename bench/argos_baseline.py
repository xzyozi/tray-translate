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
    import argostranslate.package

    argostranslate.package.update_package_index()
    available = argostranslate.package.get_available_packages()
    package = next((p for p in available if p.from_code == from_code and p.to_code == to_code), None)
    if package is None:
        raise SystemExit(f"package not found: {from_code} -> {to_code}")
    argostranslate.package.install_from_path(package.download())


def percentile(values: list[float], ratio: float) -> float:
    ordered = sorted(values)
    index = min(len(ordered) - 1, int(len(ordered) * ratio))
    return ordered[index]


def run(args: argparse.Namespace) -> dict[str, Any]:
    sentences: list[dict[str, str]] = json.loads(args.input.read_text(encoding="utf-8"))["sentences"]

    if args.install:
        install_package(args.from_code, args.to_code)

    import argostranslate.translate

    # 初回呼び出しにはモデルの読み込みが含まれるため、別に計測する
    first = sentences[0][args.from_code]
    start = time.perf_counter()
    argostranslate.translate.translate(first, args.from_code, args.to_code)
    cold_start_sec = time.perf_counter() - start

    results: list[dict[str, Any]] = []
    for sentence in sentences:
        text = sentence[args.from_code]
        timings: list[float] = []
        translated = ""
        for _ in range(args.repeat):
            start = time.perf_counter()
            translated = argostranslate.translate.translate(text, args.from_code, args.to_code)
            timings.append(time.perf_counter() - start)
        results.append(
            {
                "id": sentence["id"],
                "category": sentence["category"],
                "source": text,
                "reference": sentence.get(args.to_code),
                "output": translated,
                "chars": len(text),
                "median_sec": statistics.median(timings),
            }
        )

    medians = [r["median_sec"] for r in results]
    total_chars = sum(r["chars"] for r in results)
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
        },
        "results": results,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--from", dest="from_code", required=True, help="翻訳元の言語コード(例: en)")
    parser.add_argument("--to", dest="to_code", required=True, help="翻訳先の言語コード(例: ja)")
    parser.add_argument("--input", type=Path, default=DEFAULT_INPUT, help="テスト文セットの JSON")
    parser.add_argument("--output", type=Path, default=None, help="結果の保存先(既定: bench_<from>_<to>.json)")
    parser.add_argument("--repeat", type=int, default=5, help="1文あたりの繰り返し回数")
    parser.add_argument("--install", action="store_true", help="言語パッケージを取得してインストールする")
    args = parser.parse_args()

    output = args.output or Path(f"bench_{args.from_code}_{args.to_code}.json")
    report = run(args)
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")

    summary = report["summary"]
    print(f"pair={report['pair']} cold_start={summary['cold_start_sec']:.2f}s median={summary['median_sec']:.3f}s")
    print(f"p95={summary['p95_sec']:.3f}s chars/s={summary['chars_per_sec']:.1f} peak_rss_mb={summary['peak_rss_mb']}")
    print(f"saved: {output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
