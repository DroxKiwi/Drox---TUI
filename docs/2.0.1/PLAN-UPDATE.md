# Plan — notifications de mise a jour (opt-in)

**Statut :** brouillon  
**Ligne produit :** `2.0.1`  
**Depot releases :** [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR)  
**Principe :** **opt-in strict** — compatible certification locale (aucune requete reseau sans consentement explicite).

---

## Objectif

Informer l''utilisateur qu''une nouvelle version du **binaire TUI** est disponible sur le depot OR, via un **bandeau non bloquant** dans le REPL, avec :

- raccourci pour **accepter** (telechargement + installation),
- choix **plus tard** (snooze),
- possibilite d''**ignorer** une version,
- activation/desactivation dans les preferences.

Le moteur agent (`drox-engine`) n''est **pas** concerne : tout vit dans `drox-tui`.

---

## Contrainte locale (non negociable)

| Comportement | Par defaut |
|---|---|
| Requete reseau au demarrage | **Non** |
| Telemetrie / analytics Drox | **Non** |
| Phone home | **Non** |
| Verification MAJ | **Uniquement** si l''utilisateur l''active ou lance `/update check` |

La doc README (« certification locale ») reste vraie : **aucune connexion sortante tant que l''opt-in MAJ n''est pas active**.

---

## Source de verite des versions

### Locale

- `CARGO_PKG_VERSION` compile dans le binaire (ex. `2.0.1`).
- Optionnel : chemin d''installation enregistre dans `tui-preferences.json` apres `install.ps1` / `install.sh`.

### Distante (depot OR)

Fichier statique **`releases/latest.json`** (genere par `packaging/publish-or.ps1` a chaque release) :

```json
{
  "version": "2.0.2",
  "published_at": "2026-06-20T12:00:00Z",
  "product": "drox-tui",
  "engine_baseline": "1.5.0",
  "windows_x64": {
    "url": "https://github.com/DroxKiwi/Drox---TUI---OR/releases/download/v2.0.2/drox-tui-2.0.2-windows-x64.zip",
    "sha256": "..."
  },
  "linux_x64": {
    "url": "...",
    "sha256": "..."
  },
  "release_notes": "https://github.com/DroxKiwi/Drox---TUI---OR/blob/main/releases/v2.0.2/RELEASE_NOTES.md"
}
```

Comparaison semver simple (`2.0.2` > `2.0.1`). Pas de dependance a l''API GitHub Releases (fallback possible plus tard).

URL raw GitHub :

`https://raw.githubusercontent.com/DroxKiwi/Drox---TUI---OR/main/releases/latest.json`

---

## Preferences utilisateur

Extension de `~/.drox/tui-preferences.json` :

```json
{
  "update": {
    "enabled": false,
    "check_on_startup": false,
    "check_interval_hours": 24,
    "channel": "stable",
    "snooze_until": null,
    "dismissed_version": null,
    "install_path": null
  }
}
```

| Champ | Role |
|---|---|
| `enabled` | Master switch opt-in (desactive = jamais de check auto) |
| `check_on_startup` | Si `enabled`, verifier au lancement (max 1 fois / intervalle) |
| `check_interval_hours` | Delai minimum entre deux checks auto (defaut 24) |
| `channel` | `stable` uniquement en v1 |
| `snooze_until` | ISO8601 — masquer le bandeau jusqu''a cette date |
| `dismissed_version` | Version ignoree jusqu''a une version superieure |
| `install_path` | Chemin du binaire installe (pour remplacement auto) |

Activation via `/settings`, `/update on`, ou toggle dans onboarding avance.

---

## UX TUI

### Bandeau (non bloquant)

Affiche sous le header ou dans la zone `StatusNotice`, **sans bloquer** le composer ni l''agent :

```
[!] Mise a jour 2.0.2 disponible — Ctrl+Shift+U installer · u plus tard · /update
```

| Action | Entree |
|---|---|
| **Installer** | `Ctrl+Shift+U` puis confirmation modal (Entree) |
| **Plus tard** | `u` sur bandeau actif, ou `/update snooze` (7 jours defaut) |
| **Ignorer vX** | `/update dismiss` |
| **Verifier** | `/update check` (force, meme si opt-in off) |
| **Desactiver** | `/update off` |

Le bandeau disparait si : version a jour, snooze actif, version dismissée, ou opt-in desactive sans check manuel en cours.

### Modal confirmation (apres Ctrl+Shift+U)

```
 Mise a jour 2.0.1 -> 2.0.2
 Notes : correctifs permissions, bandeau MAJ...
 SHA256 : abc123...
 Entree = telecharger et installer · Esc = annuler
```

Corps scrollable + pied fixe (meme pattern que modal Question).

### Etats internes (`UpdateState`)

```
Idle -> Checking -> UpdateAvailable | UpToDate | CheckFailed
UpdateAvailable -> Downloading -> Verifying -> ReadyToRestart | Failed
ReadyToRestart -> (script relance) -> exit TUI
```

Poll async dans la boucle `run()` (comme `poll_ai_server_test`), sans bloquer le rendu.

---

## Flux « Accepter la mise a jour »

### Commun

1. GET `latest.json` (HTTPS GitHub raw).
2. Comparer version ; selectionner artefact OS/arch (`windows_x64` / `linux_x64`).
3. Telecharger zip/tar.gz dans `%TEMP%/drox-tui-update/` ou `/tmp/drox-tui-update/`.
4. Verifier **SHA256** (obligatoire).
5. Extraire `drox-tui.exe` / `drox-tui`.

### Windows

Si `install_path` connu (ex. `%LOCALAPPDATA%\Programs\DroxTUI\bin\drox-tui.exe`) :

1. Ecrire `drox-tui-update.ps1` qui attend la fin du PID courant, remplace le binaire, relance avec les memes args.
2. Message systeme : « Relance dans 3 s… »
3. `exit(0)`.

Sinon : message « Extraire manuellement depuis … » + chemin du zip.

### Linux

Meme logique avec script shell + `install.sh` ou remplacement `~/.local/bin/drox-tui`.

### Echec

- Erreur reseau / SHA256 : toast + ligne systeme, pas de crash.
- Bandeau repasse en « echec — /update retry ».

---

## Commandes slash

| Commande | Effet |
|---|---|
| `/update` | Aide + etat courant |
| `/update check` | Verification immediate (1 requete HTTPS) |
| `/update on` | `enabled=true`, `check_on_startup=true` |
| `/update off` | Desactive checks auto |
| `/update snooze [jours]` | Defaut 7 jours |
| `/update dismiss` | Ignore la version distante courante |
| `/update install` | Equivalent Ctrl+Shift+U si MAJ disponible |

---

## Securite

- **HTTPS uniquement** (GitHub).
- **SHA256 obligatoire** avant remplacement binaire.
- Pas de mise a jour silencieuse : **confirmation clavier explicite**.
- Phase 2 (optionnelle) : signature ed25519 de `latest.json`, cle publique embarquee dans le binaire.
- Logs : URL consultee + resultat dans `~/.drox/tui.log` (`-v`), jamais sur stderr TUI.

---

## Integration packaging OR

A chaque `publish-or.ps1` :

1. Mettre a jour `releases/latest.json` (version, URLs assets, SHA256).
2. Commit + push OR avec les binaires.

Le TUI ne depend que de `latest.json` + assets de release.

---

## Phasage implementation

| Phase | Livrable | Reseau |
|---|---|---|
| **M1** | `latest.json` genere ; `/update check` affiche resultat dans le fil | Sur demande |
| **M2** | Prefs `update.*` ; opt-in `/update on|off` | Sur demande ou startup si enabled |
| **M3** | Bandeau + snooze + dismiss + raccourcis | Idem |
| **M4** | Telechargement + SHA256 + instructions / dossier | Sur acceptation utilisateur |
| **M5** | Remplacement auto + relance (chemin install connu) | Sur acceptation utilisateur |
| **M6** | Linux parite ; tests CI | Idem |

**Ne pas implementer M4/M5 avant validation UX du bandeau (M3).**

---

## Fichiers code prevus (drox-tui)

| Fichier | Role |
|---|---|
| `engine/update.rs` | fetch, parse, semver, SHA256, download |
| `app/state.rs` | `UpdateState`, bandeau actif |
| `app/run.rs` | `poll_update_check`, raccourcis, slash |
| `widgets/update_banner.rs` | rendu bandeau |
| `widgets/update_modal.rs` | confirmation install |
| `engine/preferences.rs` | persistance `update` |
| `slash/update.rs` | commandes `/update` |

Keybinding : `BindingAction::CheckUpdate` / `ApplyUpdate` dans `keybindings.json`.

---

## Tests

- Parse `latest.json` (fixtures).
- Semver : `2.0.2` > `2.0.1`, ignore pre-release en v1.
- Snooze / dismiss logic sans reseau.
- Pas de test E2E reseau en CI (mock HTTP).

---

## Acceptation (M3)

- [ ] Par defaut : **aucune** requete au demarrage.
- [ ] `/update on` + relance : bandeau si version OR superieure.
- [ ] `u` / snooze : bandeau masque 7 jours.
- [ ] `/update dismiss` : masque jusqu''a version suivante.
- [ ] `/update check` fonctionne meme si opt-in off.
- [ ] README et certification locale mis a jour (1 phrase sur opt-in MAJ).

---

## References

- [`BUILD.md`](BUILD.md) — compilation et packaging
- [`../packaging/README.md`](../../packaging/README.md) — pipeline OR
- [Depot OR](https://github.com/DroxKiwi/Drox---TUI---OR)