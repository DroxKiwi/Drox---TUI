# Plan — `/update` et notifications MAJ (ligne 2.0.3)

> Formalise et implémente [`docs/2.0.1/PLAN-UPDATE.md`](../2.0.1/PLAN-UPDATE.md).  
> **Problème actuel** : `/update` est documenté dans le README mais **absent du code** (pas de handler slash, pas d'entrée palette, pas de module `engine/update.rs`).

**Statut** : plan  
**Dépôt releases** : [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR)

---

## Objectif

Permettre à l'utilisateur de **vérifier**, **activer** et **installer** une mise à jour du binaire TUI depuis le dépôt OR, en **opt-in strict**.

---

## Diagnostic (état 2.0.2)

| Attendu (README) | Réel |
|---|---|
| `/update`, `/update on`, `/update check` | ❌ commande inconnue → message « commande inconnue » |
| Entrée dans palette `/` | ❌ absente de `slash/palette.rs` |
| Entrée dans `/help` / composer help | ❌ non listée |
| Prefs `update.*` dans `tui-preferences.json` | ❌ non modélisées |
| `releases/latest.json` sur OR | ❌ non généré par `publish-or.ps1` |
| Bandeau `StatusNotice` | ❌ non implémenté |

---

## Périmètre 2.0.3 (jalons)

### M1 — Visibilité commande (rapide, peut livrer en premier)

- [ ] Entrée palette : `/update` + clé i18n `slash.palette.update`
- [ ] Handler `slash/update.rs` + branche dans `slash.rs`
- [ ] `/update` sans args → aide + état prefs (enabled, dernière vérif, version locale)
- [ ] Sous-commandes stub documentées : `check`, `on`, `off`, `snooze`, `dismiss`, `install`
- [ ] Entrée dans `COMPOSER_HELP_LINES` ou texte `/help` body

### M2 — Vérification distante

- [ ] Module `engine/update.rs` : fetch `latest.json`, parse semver, compare à `CARGO_PKG_VERSION`
- [ ] URL : `https://raw.githubusercontent.com/DroxKiwi/Drox---TUI---OR/main/releases/latest.json`
- [ ] `/update check` — 1 requête HTTPS même si opt-in off
- [ ] Génération `releases/latest.json` dans `packaging/publish-or.ps1` (Windows + Linux assets)

### M3 — Opt-in et bandeau

- [ ] Struct `UpdatePrefs` dans `TuiPreferences` + migration défauts
- [ ] `/update on|off`, `snooze [jours]`, `dismiss`
- [ ] Check au démarrage si `enabled` + intervalle respecté
- [ ] Bandeau non bloquant (`StatusNotice` ou widget dédié) + raccourci `Ctrl+Shift+U`
- [ ] Toggle optionnel dans modale `/settings` (ligne « Mises à jour »)

### M4 — Installation (Windows prioritaire)

- [ ] Téléchargement asset plateforme (`windows_x64` / `linux_x64`) + vérif SHA256
- [ ] Windows : remplacement binaire ou relance installateur (selon `install_path`)
- [ ] Linux : instructions fil système + `install.sh` (auto si chemin connu)
- [ ] Modal confirmation avant écriture disque

---

## API slash cible

| Commande | Effet |
|---|---|
| `/update` | Aide + état courant |
| `/update check` | Vérification immédiate OR |
| `/update on` | `enabled=true`, `check_on_startup=true` |
| `/update off` | Désactive checks auto |
| `/update snooze [jours]` | Masque bandeau (défaut 7 j) |
| `/update dismiss` | Ignore version distante courante |
| `/update install` | Lance install si MAJ disponible |

---

## `latest.json` (généré à chaque release OR)

```json
{
  "version": "2.0.3",
  "published_at": "2026-06-20T12:00:00Z",
  "product": "drox-tui",
  "engine_baseline": "1.5.0",
  "windows_x64": {
    "url": "https://github.com/DroxKiwi/Drox---TUI---OR/releases/download/v2.0.3/drox-tui-2.0.3-windows-x64-setup.exe",
    "sha256": "..."
  },
  "linux_x64": {
    "url": "https://github.com/DroxKiwi/Drox---TUI---OR/releases/download/v2.0.3/drox-tui-2.0.3-linux-x64.tar.gz",
    "sha256": "..."
  },
  "release_notes": "https://github.com/DroxKiwi/Drox---TUI---OR/blob/main/releases/v2.0.3/RELEASE_NOTES.md"
}
```

---

## Fichiers à créer / modifier

| Fichier | Rôle |
|---|---|
| `slash/update.rs` | Handler commandes |
| `slash/palette.rs` | Entrée `/update` |
| `slash.rs` | Route + `SlashOutcome::Update` |
| `engine/update.rs` | Fetch, semver, download, SHA256 |
| `engine/preferences.rs` | `UpdatePrefs` |
| `widgets/update_banner.rs` | Rendu bandeau (optionnel M3) |
| `app/run.rs` | Startup check, raccourci Ctrl+Shift+U |
| `i18n/keys*.rs` | Chaînes FR/EN |
| `packaging/publish-or.ps1` | `latest.json` + Linux dans pipeline OR |

---

## Tests

- `cargo test -p drox-tui update` — parse JSON, semver, prefs
- Test manuel : `/update` visible dans palette ; `/update check` hors-ligne → message clair
- Test manuel opt-in : aucune requête au boot tant que `enabled=false`

---

## Critères d'acceptation

- [ ] `/update` apparaît dans la palette slash et répond (pas « commande inconnue »).
- [ ] README et comportement réel alignés.
- [ ] Aucune requête réseau MAJ au démarrage par défaut.
- [ ] `latest.json` publié sur OR à chaque release 2.0.3+.
