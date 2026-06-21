# Compilation et installation — Drox TUI `2.0.2`

Produire un exécutable **release** pour distribution Windows ou Linux.

---

## 1. Prérequis

### Rust

```sh
cd drox
rustup show   # >= 1.85 (rust-toolchain.toml)
```

### Windows

- **Visual Studio Build Tools 2022** — « Développement Desktop en C++ »
- **Windows Terminal** recommandé
- Optionnel : **Ollama** pour l'inférence locale
- Installateur : **Inno Setup 6** (auto via `packaging/windows/ensure-inno.ps1`)

### Linux

```sh
sudo apt install build-essential pkg-config libssl-dev
```

---

## 2. Build release

```sh
cd drox
cargo build --release -p drox-tui
```

| OS | Binaire |
|---|---|
| Windows | `drox\target\release\drox-tui.exe` |
| Linux | `drox/target/release/drox-tui` |

---

## 3. Installateur Windows (OR)

Depuis la racine du dépôt :

```powershell
.\packaging\build-and-pack.ps1
```

Sortie : `dist\drox-tui-2.0.2-windows-x64-setup.exe`

Publication vers le dépôt OR :

```powershell
.\packaging\publish-or.ps1
# ou : .\packaging\publish-or.ps1 -OrRepo "C:\chemin\Drox---TUI---OR"
```

---

## 4. Vérification

```sh
cargo test -p drox-tui
drox-tui --list-sessions
drox-tui --workspace . -v
```

---

## 5. Confidentialité réseau

- Aucune télémétrie Drox dans le binaire.
- Connexion réseau uniquement vers **votre** serveur LLM configuré.
- Outils web / MCP : réseau uniquement si l'agent les invoque et si les permissions l'autorisent.
