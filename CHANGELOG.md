# Changelog

## 1.0.2 - 2026-09-29

- Documentation-only release; runtime source and dependency pins are unchanged
  from 1.0.1.
- Use the same September 29 six-engine speed and quality comparison in all six
  library READMEs, with consistent units, measured versions and timing boundaries.
- Include AGENTS.md with instructions for keeping README, UPSTREAM, CHANGELOG,
  release notes and crate documentation consistent.
- Package the revised documentation on crates.io. Benchmark rows retain the
  versions actually measured; this release introduces no new measurements.

## crates.io Publication - 2026-09-23

- Publish `rust-domdistiller` 1.0.1 on crates.io with the same runtime sources
  as the existing GitHub release. Include this changelog in the archive and
  update registry installation instructions; the original tag is unchanged.

## Documentation - 2026-09-23

- Refresh README quality and six-engine speed comparisons from the published
  [benchmark JSON](https://github.com/markusmobius/content-extractor-benchmark/blob/d433ab637f0a56c0926aa3698f470794a553472f/go_rust_shared_performance_2026_09_23.json),
  with separate metadata scores and exact provenance in [UPSTREAM.md](UPSTREAM.md).
- DomDistiller text F1 is 86.74080% / 92.74280% / 74.39696% on LegoNews /
  ScrapingHub / WCXB. Selected Go/Rust extraction is 3.618 / 1.965 ms/page
  (1.84x); all-four means are 3.628 / 1.973 ms/page. Shared parsing is separate.
- Rust-DomDistiller remains 1.0.1. No source, dependency, fixture or tag changes;
  these suite results use the shared Readability parser, not the standalone reader.