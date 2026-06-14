#!/usr/bin/env bash
# Install gamedev-cli from the platform manifest.
set -euo pipefail

PLATFORM="${1:-https://gamedev.jinxwashere.com}"
INSTALL_DIR="${GAMEDEV_CLI_HOME:-$HOME/.local/bin}"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) KEY=linux-x86_64 ;;
  Linux-aarch64|Linux-arm64) KEY=linux-aarch64 ;;
  Darwin-x86_64) KEY=macos-x86_64 ;;
  Darwin-arm64) KEY=macos-aarch64 ;;
  *) echo "unsupported platform: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

MANIFEST_URL="${PLATFORM%/}/tools/gamedev-cli/manifest.json"
echo "Fetching ${MANIFEST_URL}"
MANIFEST="$(curl -fsSL "${MANIFEST_URL}")"
URL="$(echo "${MANIFEST}" | python3 -c "import json,sys,os; m=json.load(sys.stdin); k=os.environ['KEY']; print(m['assets'][k]['url'])")"
SHA="$(echo "${MANIFEST}" | python3 -c "import json,sys,os; m=json.load(sys.stdin); k=os.environ['KEY']; print(m['assets'][k]['sha256'])")"
VERSION="$(echo "${MANIFEST}" | python3 -c "import json,sys; print(json.load(sys.stdin)['version'])")"
export KEY

if [[ "${URL}" != http* ]]; then
  URL="${PLATFORM%/}${URL}"
fi

TMP="$(mktemp)"
trap 'rm -f "${TMP}"' EXIT
curl -fsSL "${URL}" -o "${TMP}"
ACTUAL="$(sha256sum "${TMP}" | awk '{print $1}')"
if [[ "${ACTUAL}" != "${SHA}" ]]; then
  echo "checksum mismatch: expected ${SHA} got ${ACTUAL}" >&2
  exit 1
fi

mkdir -p "${INSTALL_DIR}"
tar -xzf "${TMP}" -C "${INSTALL_DIR}" 2>/dev/null || {
  # zip fallback
  unzip -qo "${TMP}" -d "${INSTALL_DIR}"
}
chmod +x "${INSTALL_DIR}/gamedev" 2>/dev/null || true
ln -sf "${INSTALL_DIR}/gamedev" "${INSTALL_DIR}/gamedev-cli" 2>/dev/null || true

echo "Installed gamedev-cli ${VERSION} to ${INSTALL_DIR}/gamedev"
echo "Ensure ${INSTALL_DIR} is on your PATH"
