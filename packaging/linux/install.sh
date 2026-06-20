#!/usr/bin/env bash
# Installe drox-tui (release officielle Drox TUI).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE="${SCRIPT_DIR}/drox-tui"
SYSTEM=0
PREFIX="${HOME}/.local"

usage() {
    echo "Usage: $0 [--system] [--prefix DIR]"
    echo "  default: install to ~/.local/bin/drox-tui"
    echo "  --system: install to /usr/local/bin (sudo)"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --system) SYSTEM=1; shift ;;
        --prefix) PREFIX="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) echo "Option inconnue: $1"; usage ;;
    esac
done

if [[ ! -f "$SOURCE" ]]; then
    echo "Erreur: drox-tui introuvable dans ${SCRIPT_DIR}" >&2
    exit 1
fi

if [[ "$SYSTEM" -eq 1 ]]; then
    DEST="/usr/local/bin/drox-tui"
    echo "Installation systeme: ${DEST} (sudo requis)"
    sudo install -m 755 "$SOURCE" "$DEST"
else
    DEST="${PREFIX}/bin/drox-tui"
    mkdir -p "${PREFIX}/bin"
    install -m 755 "$SOURCE" "$DEST"
    if [[ ":$PATH:" != *":${PREFIX}/bin:"* ]]; then
        echo ""
        echo "Ajoutez a votre shell (~/.bashrc ou ~/.zshrc):"
        echo "  export PATH=\"\${HOME}/.local/bin:\$PATH\""
    fi
fi

if [[ -f "${SCRIPT_DIR}/VERSION" ]]; then
    mkdir -p "${PREFIX}/share/drox-tui"
    cp "${SCRIPT_DIR}/VERSION" "${PREFIX}/share/drox-tui/VERSION"
fi

echo ""
echo "Drox TUI installe: ${DEST}"
echo "Lancement: drox-tui --workspace ~/votre-projet"
echo ""
