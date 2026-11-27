#!/usr/bin/env python3
"""Fetch and verify the MediaWiki release used by the compatibility tests."""

import hashlib
from pathlib import Path
import shutil
import tarfile
from urllib.request import Request, urlopen


VERSION = "1.46.2"
SHA256 = "8f7f937f8bbc1acd4cef1c5362a95f248dcd242020f2588798eb2f73ac2e69ba"
FILENAME = f"mediawiki-{VERSION}.tar.gz"
URL = f"https://releases.wikimedia.org/mediawiki/1.46/{FILENAME}"
USER_AGENT = "rWikiText/1.0 (https://github.com/librewiki/rWikiText)"
TARGET = Path(__file__).resolve().parents[1] / "target" / "upstream"


def main():
    TARGET.mkdir(parents=True, exist_ok=True)
    archive = TARGET / FILENAME
    if not archive.exists():
        download = archive.with_suffix(".part")
        request = Request(URL, headers={"User-Agent": USER_AGENT})
        with urlopen(request, timeout=60) as response, download.open("wb") as destination:
            shutil.copyfileobj(response, destination)
        download.replace(archive)

    with archive.open("rb") as source:
        actual = hashlib.file_digest(source, "sha256").hexdigest()
    if actual != SHA256:
        raise SystemExit(f"SHA-256 mismatch for {archive}: {actual}")

    with tarfile.open(archive) as release:
        release.extractall(TARGET, filter="data")
    print(f"Verified and extracted MediaWiki {VERSION} into {TARGET}")


if __name__ == "__main__":
    main()
