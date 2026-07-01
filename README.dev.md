# Drox TUI — dépôt de développement

Ce dépôt contient les **sources** et la doc interne. Les utilisateurs finaux reçoivent uniquement les **binaires** et le [`README.md`](README.md) public via [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR).

## Doc interne

| Chemin | Contenu |
|---|---|
| [`docs/2.0.5/`](docs/2.0.5/) | Ligne active — connexions LLM self-hosted & cloud |
| [`docs/2.0.4/`](docs/2.0.4/) | Diff visuel overlay (clôturée) |
| [`docs/2.0.6/`](docs/2.0.6/) | Multi-pane + `drox-observe` (plan) |
| [`docs/2.0.7/`](docs/2.0.7/) | Code signing & Linux (plan) |
| [`docs/2.0.3/`](docs/2.0.3/) | `/update`, Linux (clôturée) |
| [`docs/2.0.2/`](docs/2.0.2/) | Phase UI (clôturée) |
| [`packaging/`](packaging/) | Scripts build, installateurs, [`RELEASE.md`](packaging/RELEASE.md) |
| [`drox/`](drox/) | Workspace Rust (crates moteur + TUI) |

## Build release

Guide détaillé : [`packaging/RELEASE.md`](packaging/RELEASE.md).

```powershell
cd drox
cargo build --release -p drox-tui

# Windows + OR
.\packaging\publish-or.ps1 -SkipLinux

# Linux (WSL) + OR — complément
.\packaging\publish-linux-or.ps1
```

Copie binaires + `README.md` public vers `../Drox---TUI---OR`.

Branche active : **`2.0.5`** · release publique OR : **2.0.5** (Windows + Linux)
