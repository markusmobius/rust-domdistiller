import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
from datetime import datetime, timezone


ROOT = Path(__file__).resolve().parents[1]
MODES = ("skip", "prev-next", "page-number")


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def metrics(counts):
    positive = counts["true_positives"]
    missed = counts["false_negatives"]
    unwanted = counts["false_positives"]
    negative = counts["true_negatives"]
    return {
        "precision": positive / (positive + unwanted),
        "recall": positive / (positive + missed),
        "accuracy": (positive + negative) / (positive + negative + missed + unwanted),
        "f1": 2 * positive / (2 * positive + missed + unwanted),
    }


class Worker:
    def __init__(self, name, command, cwd):
        self.name = name
        self.process = subprocess.Popen(
            command,
            cwd=cwd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            env={**os.environ, "GOWORK": "off", "GOMAXPROCS": "1", "CGO_ENABLED": "0"},
        )
        try:
            self.ready = self.read()
        except BaseException:
            self.process.kill()
            self.process.wait()
            raise

    def read(self):
        line = self.process.stdout.readline()
        if not line:
            raise RuntimeError(f"{self.name} worker terminated: {self.process.poll()}")
        return json.loads(line)

    def run(self, mode, outputs=False):
        self.process.stdin.write(json.dumps({"pagination": mode, "outputs": outputs}) + "\n")
        self.process.stdin.flush()
        result = self.read()
        if result["errors"]:
            raise RuntimeError(f"{self.name}/{mode}: {result['errors']}")
        return result

    def close(self):
        try:
            self.process.stdin.close()
            self.process.wait(timeout=30)
        except (BrokenPipeError, subprocess.TimeoutExpired):
            self.process.kill()
            self.process.wait()
        finally:
            self.process.stdout.close()
        if self.process.returncode:
            raise RuntimeError(f"{self.name} exited with {self.process.returncode}")


def compare_outputs(go, rust):
    if len(go) != len(rust):
        raise ValueError("different output counts")
    differences = []
    for reference, actual in zip(go, rust):
        if reference["file"] != actual["file"]:
            raise ValueError("different corpus order")
        fields = [field for field in ("text", "title", "words", "html", "next_page", "prev_page") if reference[field] != actual[field]]
        if fields:
            differences.append({"file": reference["file"], "fields": fields})
    return differences


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, ensure_ascii=True, indent=2)
        stream.write("\n")


def main():
    parser = argparse.ArgumentParser(description="Pinned Go/Rust DomDistiller quality and speed comparison")
    parser.add_argument("--corpus", type=Path, default=ROOT / "target/benchmark/corpus.json")
    parser.add_argument("--output", type=Path, default=ROOT / "target/benchmark/results.json")
    parser.add_argument("--samples", type=int, default=10)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--mode", choices=MODES, action="append")
    parser.add_argument("--cpu", type=int)
    parser.add_argument("--limit", type=int)
    parser.add_argument("--rust-baseline", type=Path)
    args = parser.parse_args()
    if args.samples < 0 or args.warmups < 0:
        parser.error("sample and warmup counts must be nonnegative")
    if args.limit is not None and args.limit <= 0:
        parser.error("limit must be positive")
    corpus = args.corpus.resolve()
    with corpus.open(encoding="utf-8") as stream:
        pages = json.load(stream)
    if args.limit:
        pages = pages[:args.limit]
        corpus = ROOT / "target/benchmark/subset.json"
        save(corpus, pages)
    if args.cpu is not None:
        os.sched_setaffinity(0, {args.cpu})
    source_files = sorted(ROOT.glob("src/**/*.rs")) + sorted(ROOT.glob("tools/go-reference/*.go"))
    source_files += [ROOT / name for name in ("Cargo.toml", "Cargo.lock", "examples/benchmark.rs", "tools/benchmark.py", "tools/go-reference/go.mod", "tools/go-reference/go.sum")]
    source_hashes = {str(path.relative_to(ROOT)): sha256(path) for path in source_files}
    go_binary = ROOT / "target/benchmark/go-worker"
    rust_binary = ROOT / "target/release/examples/benchmark"
    baseline_binary = args.rust_baseline.resolve() if args.rust_baseline else None
    rustc = Path.home() / ".cargo/bin/rustc"
    command_environment = {**os.environ, "GOTOOLCHAIN": "go1.27.1", "GOWORK": "off"}
    results = {
        "created_utc": datetime.now(timezone.utc).isoformat(),
        "corpus_commit": "466fdbee8a504441eb78ed11d71c1da220681cab",
        "go_commit": "25b8d046ffb4053bf68345d6fa59bc9ae1961ad8",
        "corpus_sha256": sha256(corpus),
        "pages": len(pages),
        "positive_snippets": sum(len(page["with"]) for page in pages),
        "negative_snippets": sum(len(page["without"]) for page in pages),
        "platform": platform.platform(),
        "cpu": json.loads(subprocess.check_output(["lscpu", "--json"], text=True)),
        "go_version": subprocess.check_output(["go", "version"], text=True, env=command_environment).strip(),
        "rust_version": subprocess.check_output([str(rustc), "-Vv"], text=True, cwd=ROOT).strip(),
        "source_sha256": source_hashes,
        "binary_sha256": {"go": sha256(go_binary), "rust": sha256(rust_binary)},
        "affinity": sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
        "python": sys.version,
        "warmups": args.warmups,
        "samples": args.samples,
        "timing_scope": "pre-parsed DOM extraction and case-sensitive snippet scoring; excludes I/O, decoding, parsing, JSON and HTML serialization",
        "modes": {},
    }
    if baseline_binary:
        results["binary_sha256"]["rust-baseline"] = sha256(baseline_binary)
        with (ROOT / "testdata/benchmark-results.json").open(encoding="utf-8") as stream:
            baseline_record = json.load(stream)
        if baseline_record["binary_sha256"]["rust"] == results["binary_sha256"]["rust-baseline"]:
            results["baseline_record_sha256"] = sha256(ROOT / "testdata/benchmark-results.json")
            results["baseline_source_sha256"] = baseline_record["source_sha256"]
    workers = {}
    try:
        reference_name = "rust-baseline" if baseline_binary else "go"
        if baseline_binary:
            workers[reference_name] = Worker(reference_name, [str(baseline_binary), str(corpus)], ROOT)
        else:
            workers[reference_name] = Worker(reference_name, [str(go_binary), "-benchmark", str(corpus)], ROOT / "tools/go-reference")
        workers["rust"] = Worker("rust", [str(rust_binary), str(corpus)], ROOT)
        for name, worker in workers.items():
            if worker.ready != {"ready": len(pages)}:
                raise ValueError(f"unexpected {name} initialization: {worker.ready}")
        for mode in args.mode or MODES:
            quality = {}
            mode_result = {"quality": {}, "samples": [], "warmups": []}
            for name, worker in workers.items():
                response = worker.run(mode, outputs=True)
                quality[name] = response["outputs"]
                save(args.output.parent / f"{mode}-{name}.json", response)
                mode_result["quality"][name] = {"counts": response["counts"], **metrics(response["counts"])}
                print(f"{mode} {name}: {mode_result['quality'][name]}", flush=True)
            mode_result["differences"] = compare_outputs(quality[reference_name], quality["rust"])
            if baseline_binary and mode_result["differences"]:
                raise RuntimeError(f"Rust output changed: {mode_result['differences']}")
            print(f"{mode}: {len(mode_result['differences'])}/{len(pages)} pages differ", flush=True)
            mode_result["exact_pages"] = len(pages) - len(mode_result["differences"])
            for round_index in range(args.warmups + args.samples):
                order = [reference_name, "rust"] if round_index % 2 == 0 else ["rust", reference_name]
                sample = {"order": order}
                for name in order:
                    response = workers[name].run(mode)
                    if response["counts"] != mode_result["quality"][name]["counts"]:
                        raise RuntimeError(f"non-repeatable snippet counts: {mode}/{name}")
                    sample[name] = response["elapsed_ns"]
                group = "warmups" if round_index < args.warmups else "samples"
                mode_result[group].append(sample)
                print(f"{mode} {group} {len(mode_result[group])}: {reference_name}={sample[reference_name]/1e6:.1f} ms rust={sample['rust']/1e6:.1f} ms", flush=True)
            if mode_result["samples"]:
                mode_result["median_ms"] = {name: statistics.median(sample[name] for sample in mode_result["samples"]) / 1e6 for name in workers}
                mode_result["range_ms"] = {name: [min(sample[name] for sample in mode_result["samples"]) / 1e6, max(sample[name] for sample in mode_result["samples"]) / 1e6] for name in workers}
                mode_result["speedup"] = mode_result["median_ms"][reference_name] / mode_result["median_ms"]["rust"]
            results["modes"][mode] = mode_result
            save(args.output, results)
    finally:
        for worker in workers.values():
            worker.close()


if __name__ == "__main__":
    main()