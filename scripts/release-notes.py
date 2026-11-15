#!/usr/bin/env python3
"""Writes the changelog part of a release's notes: every commit since the
previous release tag, newest first, with its message body. CI-only commits
are left out. Usage: release-notes.py VERSION > notes.md"""

import re
import subprocess
import sys


def git(*args: str) -> str:
    return subprocess.run(["git", *args], capture_output=True, text=True, check=False).stdout


def main() -> None:
    version = sys.argv[1]
    prev = git("describe", "--tags", "--abbrev=0", "--match", "v*", "HEAD").strip()
    span = f"{prev}..HEAD" if prev else "HEAD"
    log = git("log", "--no-merges", "--format=%s%x1f%b%x1e", span)

    out = []
    for entry in log.split("\x1e"):
        entry = entry.strip("\n")
        if not entry:
            continue
        subject, _, body = entry.partition("\x1f")
        subject = subject.strip()
        if subject.startswith("CI:") or subject.startswith("CI "):
            continue
        out.append(f"- **{subject}**")
        lines = [l.rstrip() for l in body.strip().splitlines()]
        # Drop trailers like Co-authored-by.
        lines = [l for l in lines if not re.match(r"^[A-Z][\w-]*-by: ", l)]
        for line in lines:
            out.append(f"  {line}" if line else "")
    if not out:
        out.append("- Build and packaging changes only.")

    print(f"## What's new in {version}\n")
    print("\n".join(out))
    print()


if __name__ == "__main__":
    main()
