# Build — Drox TUI 2.0.3

## Lancer (rappel)

```bash
drox-tui --workspace /chemin/projet
```

```powershell
drox-tui --workspace C:\chemin\projet
```

## Windows x64

```powershell
.\packaging\build-and-pack.ps1
# dist\drox-tui-<version>-windows-x64-setup.exe
```

## Linux x64

```bash
./packaging/build-and-pack-linux.sh
# dist/drox-tui-<version>-linux-x64.tar.gz
```

## Publication OR (Windows + Linux)

Depuis Windows (WSL requis pour l’archive Linux) :

```powershell
.\packaging\publish-or.ps1
# -SkipLinux si pas de WSL
# -SkipBuild pour réutiliser les binaires déjà compilés
```

Copie vers `../Drox---TUI---OR` : installateur Windows, archive Linux, `latest.json`, README.

## Tests

```bash
cd drox && cargo test -p drox-tui
```

CI : `.github/workflows/ci.yml` (Ubuntu + Windows).
