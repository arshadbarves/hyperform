#!/usr/bin/env python3
"""Fail when a relative link in the documentation points at a missing file.

Checks README.md, AGENTS.md, CHANGELOG.md and every markdown file under
docs/ and .claude/. External links (http, mailto) and pure anchors are
skipped; anchors after a path are ignored. Run from anywhere:

    python3 scripts/check_docs_links.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LINK = re.compile(r"\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
FENCE = re.compile(r"^\s*(```|~~~)")
SKIP_PREFIXES = ("http://", "https://", "mailto:", "#", "tauri://", "bc-")


def markdown_files() -> list[Path]:
    files = [ROOT / name for name in ("README.md", "AGENTS.md", "CHANGELOG.md")]
    for folder in ("docs", ".claude"):
        files.extend(sorted((ROOT / folder).rglob("*.md")))
    return [path for path in files if path.is_file()]


def links(path: Path) -> list[tuple[int, str]]:
    found: list[tuple[int, str]] = []
    in_fence = False
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if FENCE.match(line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        found.extend((number, target) for target in LINK.findall(line))
    return found


def broken(path: Path) -> list[str]:
    problems = []
    for number, target in links(path):
        if target.startswith(SKIP_PREFIXES):
            continue
        relative = target.split("#", 1)[0]
        if not relative:
            continue
        resolved = (path.parent / relative).resolve()
        if not resolved.exists():
            shown = path.relative_to(ROOT)
            problems.append(f"{shown}:{number}: missing {target}")
    return problems


def main() -> int:
    problems = [problem for path in markdown_files() for problem in broken(path)]
    for problem in problems:
        print(problem)
    if problems:
        print(f"{len(problems)} broken documentation link(s)")
        return 1
    print("documentation links OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
