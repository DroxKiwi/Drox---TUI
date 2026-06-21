# Drox TUI — dépôt de développement

Ce dépôt contient les **sources** et la doc interne. Les utilisateurs finaux reçoivent uniquement les **binaires** et le [`README.md`](README.md) public via [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR).

## Doc interne

| Chemin | Contenu |
|---|---|
| [`docs/2.0.3/`](docs/2.0.3/) | Ligne active — `/update`, Linux |
| [`docs/2.0.2/`](docs/2.0.2/) | Phase UI (clôturée) |
| [`packaging/`](packaging/) | Scripts build, installateurs, `publish-or.ps1` |
| [`drox/`](drox/) | Workspace Rust (crates moteur + TUI) |

## Build release

```powershell
cd drox
cargo build --release -p drox-tui
.\packaging\publish-or.ps1
```

Copie binaires + `README.md` public vers `../Drox---TUI---OR`.

Branche active : **`2.0.3`**
