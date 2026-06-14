#!/usr/bin/env python3
"""Stage and tarball the CLI bundle baked into the production Docker image."""

from __future__ import annotations

import argparse
import importlib.util
import json
import shutil
import sys
import tarfile
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
build_assets = _manifest.build_assets
sha256_file = _manifest.sha256_file

EXPECTED_ASSETS = tuple(ASSET_NAMES.keys())


def copy_install_scripts(src_tools: Path, dest_tools: Path) -> None:
    for name in ("install.ps1", "install.sh"):
        shutil.copy2(src_tools / name, dest_tools / name)


def stage_bundle(
    *,
    version: str,
    artifacts_dir: Path,
    framework_root: Path,
    out_tar: Path,
) -> None:
    platform_manifest = framework_root / "platform" / "manifest.json"
    tools_src = framework_root / "tools" / "gamedev-cli"
    tools_out = framework_root / "release-artifacts" / "tools"
    version_dir = tools_out / f"v{version}"

    if tools_out.exists():
        shutil.rmtree(tools_out)
    tools_out.mkdir(parents=True)
    version_dir.mkdir(parents=True)

    assets = build_assets(artifacts_dir, version)
    if set(assets) != set(EXPECTED_ASSETS):
        missing = sorted(set(EXPECTED_ASSETS) - set(assets))
        raise SystemExit(f"missing CLI artifacts for: {missing}")

    # Update manifests in the real tree (used by verify + Docker build context).
    platform = json.loads(platform_manifest.read_text(encoding="utf-8"))
    platform["framework_version"] = platform.get("framework_version", version)
    platform["cli"]["version"] = version
    platform["cli"]["assets"] = assets
    platform_manifest.write_text(json.dumps(platform, indent=2) + "\n", encoding="utf-8")

    cli_manifest = {
        "version": version,
        "min_supported": platform["cli"].get("min_supported", version),
        "released_at": platform.get("released_at"),
        "assets": assets,
        "notes": platform["cli"].get("notes"),
    }
    cli_json = json.dumps(cli_manifest, indent=2) + "\n"
    (tools_src / "manifest.json").write_text(cli_json, encoding="utf-8")
    (tools_out / "manifest.json").write_text(cli_json, encoding="utf-8")
    copy_install_scripts(tools_src, tools_out)

    for key, meta in assets.items():
        filename = ASSET_NAMES[key]
        found = next(artifacts_dir.rglob(filename), None)
        if found is None:
            raise SystemExit(f"artifact not found: {filename}")
        shutil.copy2(found, version_dir / filename)

    out_tar.parent.mkdir(parents=True, exist_ok=True)
    if out_tar.exists():
        out_tar.unlink()

    # Tar paths relative to framework root so extraction is deterministic.
    with tarfile.open(out_tar, "w:gz") as tar:
        tar.add(platform_manifest, arcname="platform/manifest.json")
        tar.add(tools_out, arcname="release-artifacts/tools")
    print(f"Wrote {out_tar} ({sha256_file(out_tar)})")


def extract_bundle(*, tar_path: Path, framework_root: Path) -> None:
    with tarfile.open(tar_path, "r:gz") as tar:
        tar.extractall(path=framework_root, filter="data")


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)

    pack = sub.add_parser("pack", help="Build cli-image-bundle.tar.gz from raw CLI artifacts")
    pack.add_argument("--version", required=True)
    pack.add_argument("--artifacts", type=Path, required=True)
    pack.add_argument("--framework-root", type=Path, default=Path(__file__).resolve().parents[1])
    pack.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Output tarball (default: <framework-root>/cli-image-bundle.tar.gz)",
    )

    extract = sub.add_parser("extract", help="Extract bundle tarball into framework root")
    extract.add_argument("--tar", type=Path, required=True)
    extract.add_argument("--framework-root", type=Path, default=Path(__file__).resolve().parents[1])

    args = parser.parse_args()
    if args.command == "pack":
        out = args.out or (args.framework_root / "cli-image-bundle.tar.gz")
        stage_bundle(
            version=args.version,
            artifacts_dir=args.artifacts,
            framework_root=args.framework_root,
            out_tar=out,
        )
    else:
        extract_bundle(tar_path=args.tar, framework_root=args.framework_root)


if __name__ == "__main__":
    main()
