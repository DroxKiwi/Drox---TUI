# Drox TUI v2.0.3 — Notes de release

**Produit** : Drox TUI `2.0.3`  
**Moteur** : dérivé du moteur agent Drox IDE `1.5.0`  
**Ligne** : Mises à jour opt-in + parité Linux x64  
**Certification** : local-first — pas de télémétrie Drox, pas de cloud obligatoire.

---

## Nouveautés

### Commande `/update` (opt-in)

- Entrée dans la palette slash, `/help` et les suggestions
- **`/update on`** / **`/update off`** — activation des vérifications automatiques (désactivé par défaut)
- **`/update check`** — contrôle manuel vers le dépôt OR (`latest.json`)
- **`/update install`** — téléchargement HTTPS, vérification SHA256, modale de confirmation
- Bandeau discret + pastille header lorsqu'une MAJ est disponible
- Raccourci **`Ctrl+Shift+U`** — installer ou snooze 7 jours (`u`)
- Aucune requête réseau au démarrage sans opt-in explicite

### Linux x64 (first-class)

- Archive `drox-tui-2.0.3-linux-x64.tar.gz` + `install.sh` publiées sur OR
- Navigateur workspace : racines `$HOME`, `/`, `/mnt` (WSL)
- Installateur `/update install` sur Linux (extraction + remplacement binaire)
- CI Ubuntu + Windows (`cargo test -p drox-tui`)

### Documentation

- README : section **Lancer Drox TUI** avec `drox-tui --workspace .` dès l'intro (FR/EN)

---

## Installation

### Windows x64

1. Téléchargez `drox-tui-2.0.3-windows-x64-setup.exe` depuis [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR/releases/tag/v2.0.3)
2. Lancez l'installateur, cochez « Ajouter au PATH »
3. Nouveau terminal :

```powershell
drox-tui --workspace C:\chemin\projet
```

### Linux x64

```bash
tar xzf drox-tui-2.0.3-linux-x64.tar.gz
cd drox-tui-2.0.3-linux-x64
./install.sh
drox-tui --workspace ~/projets/mon-repo
```

---

## Migration depuis 2.0.2

- Aucune migration de prefs requise
- Les vérifications de mise à jour restent **désactivées** tant que vous n'exécutez pas `/update on`
- Sessions existantes (`ses_*.jsonl`) inchangées

---

## Raccourcis utiles

| Commande / touche | Action |
|---|---|
| `/update on` | Active les vérifications MAJ (opt-in) |
| `/update check` | Vérifie manuellement une nouvelle version |
| `/update install` | Télécharge et installe la MAJ (confirmation) |
| `Ctrl+Shift+U` | Installer la MAJ ou ouvrir le bandeau |
| `u` (bandeau) | Snooze 7 jours |

---

## Artefacts OR

| Fichier | Description |
|---|---|
| `drox-tui-2.0.3-windows-x64-setup.exe` | Installateur Windows |
| `drox-tui-2.0.3-linux-x64.tar.gz` | Archive Linux |
| `SHA256SUMS-windows.txt` | Empreinte SHA256 Windows |
| `SHA256SUMS-linux.txt` | Empreinte SHA256 Linux |
| `releases/latest.json` | Métadonnées pour `/update` |

---

## QA

- Tests automatiques : `cargo test -p drox-tui` (188 tests)
- Build release Windows + Linux via `packaging/publish-or.ps1`

---

*Date de publication : 2026-06-19*
