#!/usr/bin/env bash
# Compile drox-tui release et produit l'archive Linux x64 (.tar.gz).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DROX_DIR="${REPO_ROOT}/drox"
PACKAGING_DIR="${REPO_ROOT}/packaging"
SKIP_BUILD=0
OUT_DIR="${REPO_ROOT}/dist"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-build) SKIP_BUILD=1; shift ;;
        --out-dir) OUT_DIR="$2"; shift 2 ;;
        *) echo "Option inconnue: $1"; exit 1 ;;
    esac
done

VERSION="$(grep -A5 '^\[workspace.package\]' "${DROX_DIR}/Cargo.toml" | grep '^version' | head -1 | sed 's/.*"\(.*\)".*/\1/')"
STAGE_NAME="drox-tui-${VERSION}-linux-x64"
STAGE_DIR="${OUT_DIR}/${STAGE_NAME}"
ARCHIVE="${OUT_DIR}/${STAGE_NAME}.tar.gz"

echo "==> Drox TUI release ${VERSION} (Linux x64)"

if [[ "${SKIP_BUILD}" -eq 0 ]]; then
    (cd "${DROX_DIR}" && cargo build --release -p drox-tui)
fi

BINARY="${DROX_DIR}/target/release/drox-tui"
if [[ ! -f "${BINARY}" ]]; then
    echo "Binaire absent: ${BINARY}" >&2
    exit 1
fi

rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}"

install -m 755 "${BINARY}" "${STAGE_DIR}/drox-tui"
install -m 755 "${PACKAGING_DIR}/linux/install.sh" "${STAGE_DIR}/install.sh"
cp "${PACKAGING_DIR}/README-INSTALL.txt" "${STAGE_DIR}/"
cp "${PACKAGING_DIR}/LICENSE-MIT.txt" "${STAGE_DIR}/LICENSE"
echo "${VERSION}" > "${STAGE_DIR}/VERSION"

mkdir -p "${OUT_DIR}"
tar -czf "${ARCHIVE}" -C "${OUT_DIR}" "${STAGE_NAME}"
SHA256="$(sha256sum "${ARCHIVE}" | awk '{print $1}')"
echo "${SHA256}  ${STAGE_NAME}.tar.gz" > "${OUT_DIR}/SHA256SUMS-${VERSION}-linux.txt"

echo ""
echo "Archive: ${ARCHIVE}"
echo "SHA256:  ${SHA256}"
echo "Stage:   ${STAGE_DIR}"
echo ""
