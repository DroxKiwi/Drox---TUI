# Drox TUI v2.0.2 — Notes de release

**Produit** : Drox TUI `2.0.2`  
**Moteur** : dérivé du moteur agent Drox IDE `1.5.0`  
**Ligne** : Phase UI — charte Drox, souris, animations, i18n FR/EN, bibliothèque connexions LLM  
**Certification** : local-first — pas de télémétrie Drox, pas de cloud obligatoire.

---

## Nouveautés

### Identité visuelle Drox

- Thème **Drox** par défaut (noir profond, vert phosphore) + variante `drox-ansi` 16 couleurs
- Modales style Pip-Boy (coins arrondis, animation d’entrée)
- **Boot splash** au lancement (logo DROX, halo discret)
- Composer : curseur clignotant + bordure pulse pendant un run agent
- Pastille de statut connexion dans le header

### Interactions

- **Souris** : scroll du fil, clic sur modales Question/Permission et `/server`, hover outils
- **Animations** UI activables/désactivables (`/settings` ou modale réglages)
- Migration automatique des prefs `dark` → `drox` à l’ouverture

### Interface FR / EN

- Module `i18n` avec catalogues FR/EN et persistance `ui_locale`
- Modales P0 (connexion IA, workspace, permissions, onboarding)
- P1 : palette slash, `/help`, statuts, toasts
- Changement à chaud : `/language fr|en` ou modale **`/settings`**

### Modale réglages

- **`/settings`** ouvre une fenêtre (langue, animations, souris, vim)
- **`/settings print`** : dump texte des préférences (debug)

### Connexions LLM

- Bibliothèque de **profils** persistés (Ollama local, Ollama Cloud, vLLM, custom)
- Headers custom, Bearer, Api-Key header
- Assistant `/server` en 3 étapes + test connexion
- Migration prefs 2.0.1 → profils actifs

---

## Installation

### Windows x64

1. Téléchargez `drox-tui-2.0.2-windows-x64-setup.exe` depuis [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR/releases/tag/v2.0.2)
2. Lancez l’installateur, cochez « Ajouter au PATH »
3. Nouveau terminal :

```powershell
drox-tui --workspace C:\chemin\projet
```

### Linux x64

```bash
tar xzf drox-tui-2.0.2-linux-x64.tar.gz
cd drox-tui-2.0.2-linux-x64
./install.sh
drox-tui --workspace ~/projets/mon-repo
```

### Depuis les sources

Voir [BUILD.md](BUILD.md).

---

## Migration depuis 2.0.1

- Thème `dark` / `dark-ansi` migré vers **Drox** automatiquement
- Connexion LLM legacy importée dans la bibliothèque de profils
- Aucune action requise pour les sessions existantes (`ses_*.jsonl`)

---

## Raccourcis utiles

| Commande | Action |
|---|---|
| `/settings` | Modale réglages (langue, souris, animations, vim) |
| `/language fr\|en` | Langue interface |
| `/server` | Assistant connexion IA |
| `/theme` | Thème TUI |
| `/help` | Aide composer (bilingue) |

---

## Artefacts OR

| Fichier | Description |
|---|---|
| `drox-tui-2.0.2-windows-x64-setup.exe` | Installateur Windows |
| `SHA256SUMS-windows.txt` | Empreinte SHA256 |
| `drox-tui-2.0.2-linux-x64.tar.gz` | Archive Linux (si publiée) |

---

## QA

- Tests automatiques : `cargo test -p drox-tui` (175 tests)
- QA manuelle partielle : resume/export, permissions, local-first validés en session de test

---

*Date de publication : 2026-06-19*
