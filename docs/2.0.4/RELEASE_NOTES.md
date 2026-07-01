# Drox TUI v2.0.4 — Notes de release

**Produit** : Drox TUI `2.0.4`  
**Moteur** : dérivé du moteur agent Drox IDE `1.5.0`  
**Ligne** : Diff visuel TUI + correctifs `/update`  
**Certification** : local-first — pas de télémétrie Drox, pas de cloud obligatoire.

---

## Nouveautés

### Diff visuel (M1–M4)

- **`/diff`** — overlay scrollable avec patch unifié coloré (`git diff HEAD`)
- **`/diff --stat`** — résumé compact (mode texte)
- **`/diff <fichier>`** — diff ciblé + navigation dans la liste `git status`
- **Fil agent** — touche **`e`** sur tout diff non vide ; hint dans le fil
- **Modale permission** — **`e`** pour diff complet avant acceptation
- **Fin de run** — bandeau « N fichiers modifiés » ; **Entrée** ouvre le viewer multi-fichier
- **Thème** — couleurs diff depuis `ThemePalette` ; numéros de ligne ; word-diff
- **`file_write`** — patch unifié dans le fil (comme `file_edit`)

### `/update` (polish)

- Pastille header **`[MAJ on]`** / **`[MAJ off]`** selon opt-in
- Palette slash : sous-commandes (`/update on`, `update on` dans le filtre)
- Autocomplétion composer : `/update ` → `on`, `check`, `off`, …
- Correctif **réponses assistant en double** (routage streaming → bloc phase)

---

## Installation

### Windows x64

1. Téléchargez `drox-tui-2.0.4-windows-x64-setup.exe` depuis [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR/releases/tag/v2.0.4)
2. Lancez l'installateur, cochez « Ajouter au PATH »
3. Nouveau terminal :

```powershell
drox-tui --workspace C:\chemin\projet
```

### Linux x64

```bash
tar xzf drox-tui-2.0.4-linux-x64.tar.gz
cd drox-tui-2.0.4-linux-x64
./install.sh
drox-tui --workspace ~/projets/mon-repo
```

---

## Migration depuis 2.0.3

- Aucune migration de prefs requise
- `/update on|off` inchangé ; header affiche désormais l'état opt-in
- Sessions et transcripts conservés

---

## Raccourcis utiles

| Commande / touche | Action |
|---|---|
| `/diff` | Diff workspace unifié (overlay) |
| `/diff --stat` | Résumé stat |
| `/diff <fichier>` | Diff d'un fichier |
| `e` | Agrandir diff outil / permission |
| `Entrée` (fin de run) | Viewer diffs du run |
| `/update on` | Active vérifications MAJ + pastille header |

---

## Artefacts OR

| Fichier | Description |
|---|---|
| `drox-tui-2.0.4-windows-x64-setup.exe` | Installateur Windows |
| `drox-tui-2.0.4-linux-x64.tar.gz` | Archive Linux |
| `SHA256SUMS-windows.txt` | Empreinte SHA256 Windows |
| `SHA256SUMS-linux.txt` | Empreinte SHA256 Linux |
| `releases/latest.json` | Métadonnées pour `/update` |

---

## QA

- Tests automatiques : `cargo test -p drox-tui` (205+ tests)
- Build release : `packaging/publish-or.ps1`

---

## Reporté (lignes suivantes)

- **Code signing + GPG Linux** → [`2.0.7`](../2.0.7/README.md)
- **Connexions LLM self-hosted & cloud** → [`2.0.5`](../2.0.5/README.md)
- **Poste multi-pane + diff inline fil** → [`2.0.6`](../2.0.6/README.md)
- **Animation splash IDE** → [`animation-start`](../animation-start/README.md)

---

*Date de publication : 2026-06-19*
