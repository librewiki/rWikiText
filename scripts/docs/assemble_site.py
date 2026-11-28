#!/usr/bin/env python3
"""Assemble and verify the multi-language API documentation site."""

from __future__ import annotations

import argparse
from pathlib import Path
import shutil
import sys

LANGUAGES = ("rust", "c", "python", "node", "go", "java", "kotlin", "php")

RUST_REDIRECT_HTML = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>rWikiText Rust API</title>
  <meta http-equiv="refresh" content="0; url=rwikitext/index.html">
  <link rel="canonical" href="rwikitext/index.html">
</head>
<body>
  <p>Redirecting to <a href="rwikitext/index.html">rwikitext Rust API documentation</a>...</p>
</body>
</html>
"""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--site-dir",
        type=Path,
        default=Path("target/api-docs"),
        help="Root directory containing per-language compiled docs",
    )
    parser.add_argument(
        "--index",
        type=Path,
        default=Path("docs/api/index.html"),
        help="Landing/router index.html template path",
    )
    parser.add_argument(
        "--languages",
        nargs="+",
        default=list(LANGUAGES),
        help="Expected language subdirectories to validate",
    )
    args = parser.parse_args()

    site_dir: Path = args.site_dir
    site_dir.mkdir(parents=True, exist_ok=True)

    if not args.index.is_file():
        print(f"error: index template not found: {args.index}", file=sys.stderr)
        return 1

    shutil.copyfile(args.index, site_dir / "index.html")
    (site_dir / ".nojekyll").write_text("", encoding="utf-8")

    rust_crate_index = site_dir / "rust" / "rwikitext" / "index.html"
    rust_root_index = site_dir / "rust" / "index.html"
    if rust_crate_index.is_file() and not rust_root_index.is_file():
        rust_root_index.write_text(RUST_REDIRECT_HTML, encoding="utf-8")

    missing: list[str] = []
    for lang in args.languages:
        entry = site_dir / lang / "index.html"
        if not entry.is_file() or entry.stat().st_size == 0:
            missing.append(str(entry))

    if missing:
        print("error: missing or empty compiled documentation entrypoints:", file=sys.stderr)
        for path in missing:
            print(f"  - {path}", file=sys.stderr)
        return 1

    print(f"Assembled API docs site in {site_dir} ({len(args.languages)} languages verified).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
