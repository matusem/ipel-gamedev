#!/usr/bin/env python3
"""Generate Ed25519 key pair for the deploy webhook (store private key in GitHub Secrets)."""
from __future__ import annotations

import base64
import sys

try:
    from nacl.signing import SigningKey
except ImportError:
    print("Install PyNaCl: pip install pynacl", file=sys.stderr)
    sys.exit(1)


def main() -> None:
    key = SigningKey.generate()
    priv = base64.b64encode(bytes(key)).decode()
    pub = base64.b64encode(bytes(key.verify_key)).decode()
    print("Add to Pi .env:")
    print(f"DEPLOY_WEBHOOK_PUBLIC_KEY={pub}")
    print()
    print("Add to GitHub Actions secret DEPLOY_WEBHOOK_PRIVATE_KEY:")
    print(priv)
    print()
    print("Never commit the private key.")


if __name__ == "__main__":
    main()
