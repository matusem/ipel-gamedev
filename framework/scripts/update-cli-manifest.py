#!/usr/bin/env python3
"""Update CLI asset URLs and sha256 entries in platform + CLI manifests."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

ASSET_NAMES = {
    "windows-x86_64": "gamedev-cli-windows-x86_64.zip",
    "linux-x86_64": "gamedev-cli-linux-x86_64.tar.gz",
    "linux-aarch64": "gamedev-cli-linux-aarch64.tar.gz",
    "macos-x86_64": "gamedev-cli-macos-x86_64.tar.gz",
    "macos-aarch64": "gamedev-cli-macos-aarch64.tar.gz",
}


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def find_artifact(artifacts_dir: Path, key: str) -> Path | None:
    name = ASSET_NAMES[key]
    for p in artifacts_dir.rglob(name):
        return p
    return None


def build_assets(artifacts_dir: Path, version: str) -> dict:
    assets = {}
    for key, filename in ASSET_NAMES.items():
        path = find_artifact(artifacts_dir, key)
        if path is None:
            continue
        assets[key] = {
            "url": f"/tools/gamedev-cli/v{version}/{filename}",
            "sha256": sha256_file(path),
        }
    return assets


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--cli-out", type=Path, required=True)
    args = parser.parse_args()

    assets = build_assets(args.artifacts, args.version)

    platform = json.loads(args.out.read_text(encoding="utf-8"))
    platform["framework_version"] = platform.get("framework_version", args.version)
    platform["cli"]["version"] = args.version
    platform["cli"]["assets"] = assets
    args.out.write_text(json.dumps(platform, indent=2) + "\n", encoding="utf-8")

    cli_manifest = {
        "version": args.version,
        "min_supported": platform["cli"].get("min_supported", args.version),
        "released_at": platform.get("released_at"),
        "assets": assets,
        "notes": platform["cli"].get("notes"),
    }
    args.cli_out.write_text(json.dumps(cli_manifest, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
