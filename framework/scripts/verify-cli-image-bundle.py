#!/usr/bin/env python3
"""Verify staged CLI image bundle before Docker build."""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
_SPEC = importlib.util.spec_from_file_location(
    "update_cli_manifest", _SCRIPTS / "update-cli-manifest.py"
)
if _SPEC is None or _SPEC.loader is None:
    raise RuntimeError("failed to load update-cli-manifest.py")
_manifest = importlib.util.module_from_spec(_SPEC)
sys.modules["update_cli_manifest"] = _manifest
_SPEC.loader.exec_module(_manifest)

ASSET_NAMES = _manifest.ASSET_NAMES

PLACEHOLDER_SHA = "0" * 64
EXPECTED_ASSETS = tuple(ASSET_NAMES.keys())


def verify_bundle(framework_root: Path, version: str) -> None:
    platform_manifest = framework_root / "platform" / "manifest.json"
    cli_manifest = framework_root / "release-artifacts" / "tools" / "manifest.json"
    version_dir = framework_root / "release-artifacts" / "tools" / f"v{version}"

    for path in (platform_manifest, cli_manifest):
        if not path.is_file():
            raise SystemExit(f"missing {path}")

    for script in ("install.ps1", "install.sh"):
        path = framework_root / "release-artifacts" / "tools" / script
        if not path.is_file():
            raise SystemExit(f"missing {path}")

    if not version_dir.is_dir():
        raise SystemExit(f"missing version directory {version_dir}")

    cli = json.loads(cli_manifest.read_text(encoding="utf-8"))
    assets = cli.get("assets", {})
    if set(assets) != set(EXPECTED_ASSETS):
        missing = sorted(set(EXPECTED_ASSETS) - set(assets))
        extra = sorted(set(assets) - set(EXPECTED_ASSETS))
        raise SystemExit(f"manifest assets mismatch: missing={missing} extra={extra}")

    for key, meta in assets.items():
        sha = meta.get("sha256", "")
        if sha.startswith(PLACEHOLDER_SHA):
            raise SystemExit(f"placeholder checksum for {key}")

        filename = ASSET_NAMES[key]
        archive = version_dir / filename
        if not archive.is_file():
            raise SystemExit(f"missing archive {archive}")

        url = meta.get("url", "")
        expected_suffix = f"/tools/gamedev-cli/v{version}/{filename}"
        if not url.endswith(expected_suffix):
            raise SystemExit(f"unexpected url for {key}: {url}")

    archives = sorted(p.name for p in version_dir.iterdir() if p.is_file())
    expected = sorted(ASSET_NAMES.values())
    if archives != expected:
        raise SystemExit(f"archive set mismatch: got {archives} expected {expected}")

    print(f"CLI image bundle OK for v{version} ({len(EXPECTED_ASSETS)} platforms)")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument(
        "--framework-root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
    )
    args = parser.parse_args()
    try:
        verify_bundle(args.framework_root, args.version)
    except SystemExit as exc:
        print(exc, file=sys.stderr)
        raise


if __name__ == "__main__":
    main()
