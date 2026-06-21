# Drox TUI — dépôt de développement

Ce dépôt contient les **sources** et la doc interne. Les utilisateurs finaux reçoivent uniquement les **binaires** et le [`README.md`](README.md) public via [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR).

## Doc interne

| Chemin | Contenu |
|---|---|
| [`docs/features/`](docs/features/) | **Catalogue features portables** TUI ↔ IDE |
| [`docs/2.0.5/`](docs/2.0.5/) | Ligne active — multi-pane + observe |
| [`docs/2.0.4/`](docs/2.0.4/) | Diff visuel overlay (clôturée) |
| [`docs/2.0.6/`](docs/2.0.6/) | Code signing & Linux (plan) |
| [`docs/2.0.3/`](docs/2.0.3/) | `/update`, Linux (clôturée) |
| [`docs/2.0.2/`](docs/2.0.2/) | Phase UI (clôturée) |
| [`packaging/`](packaging/) | Scripts build, installateurs, `publish-or.ps1` |
| [`drox/`](drox/) | Workspace Rust (crates moteur + TUI) |

## Build release

```powershell
cd drox
cargo build --release -p drox-tui
.\packaging\publish-or.ps1 -SkipLinux   # Windows seul pour l'instant
```

Copie binaires + `README.md` public vers `../Drox---TUI---OR`.

Branche active : **`2.0.5`** · release publique OR : **2.0.4** (Windows)
