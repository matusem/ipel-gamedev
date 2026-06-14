#!/usr/bin/env python3
"""Sign and POST a deploy webhook (used from GitHub Actions after image push)."""
from __future__ import annotations

import argparse
import base64
import json
import os
import sys
import time
import urllib.error
import urllib.request

try:
    from nacl.signing import SigningKey
except ImportError:
    print("Install PyNaCl: pip install pynacl", file=sys.stderr)
    sys.exit(1)


def sign(private_b64: str, timestamp: str, body: bytes) -> str:
    key = SigningKey(base64.b64decode(private_b64.strip()))
    message = timestamp.encode() + b"\n" + body
    return base64.b64encode(key.sign(message).signature).decode()


def main() -> None:
    parser = argparse.ArgumentParser(description="Trigger signed deploy webhook")
    parser.add_argument("--url", default=os.environ.get("DEPLOY_WEBHOOK_URL"))
    parser.add_argument(
        "--private-key",
        default=os.environ.get("DEPLOY_WEBHOOK_PRIVATE_KEY"),
    )
    parser.add_argument(
        "--token",
        default=os.environ.get("DEPLOY_WEBHOOK_TOKEN"),
        help="Shared bypass token (DEPLOY_WEBHOOK_TOKEN); sent as X-Deploy-Token",
    )
    parser.add_argument("--image", required=True)
    parser.add_argument("--tag", required=True)
    args = parser.parse_args()

    if not args.url or not args.private_key:
        print(
            "Set --url and --private-key (or DEPLOY_WEBHOOK_URL / DEPLOY_WEBHOOK_PRIVATE_KEY)",
            file=sys.stderr,
        )
        sys.exit(1)

    body = json.dumps({"image": args.image, "tag": args.tag}, separators=(",", ":")).encode()
    timestamp = str(int(time.time()))
    signature = sign(args.private_key, timestamp, body)

    headers = {
        "Content-Type": "application/json",
        "X-Deploy-Timestamp": timestamp,
        "X-Deploy-Signature": signature,
    }
    if args.token:
        headers["X-Deploy-Token"] = args.token.strip()

    req = urllib.request.Request(
        args.url.rstrip("/") + "/internal/deploy",
        data=body,
        method="POST",
        headers=headers,
    )

    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            print(resp.status, resp.read().decode())
    except urllib.error.HTTPError as e:
        print(e.code, e.read().decode(), file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
