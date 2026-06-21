# Plan — adaptation Linux (ligne 2.0.3)

**Statut** : plan  
**Objectif** : Drox TUI **utilisable au quotidien** sur Linux x64 (terminal moderne), avec **release OR** au même niveau que Windows.

---

## État actuel (audit 2.0.2)

| Zone | Linux | Notes |
|---|---|---|
| Build `cargo build --release` | ✅ | Script `packaging/build-and-pack-linux.sh` |
| Archive `tar.gz` + `install.sh` | ✅ | Non intégrée à `publish-or.ps1` |
| Copie presse-papiers (`/copy`, Ctrl+Y) | ✅ | `wl-copy` puis `xclip` |
| Collage image presse-papiers | ❌ | Windows uniquement (`image_paste.rs`) |
| Explorateur `/workspace` | ⚠️ | Racine `/` ; pas de `$HOME` dans roots |
| Souris (scroll, clic modales) | ⚠️ | crossterm — à valider GNOME/Konsole, WT via WSL |
| Icône application | N/A | Pas d'icône embarquée Linux (normal) |
| `winres` / `.ico` | N/A | `build.rs` no-op hors Windows |
| Doctor / bash | ✅ | Shell POSIX |
| Chemins `~/.drox` | ✅ | `dirs` crate |

**Constat** : le TUI **compile et tourne** sur Linux, mais l'expérience produit (packaging OR, UX presse-papiers/souris, doc) reste orientée Windows.

---

## Périmètre 2.0.3

### P0 — Release OR Linux

- [ ] Intégrer `build-and-pack-linux.sh` dans `publish-or.ps1` (ou script sibling `publish-or-linux.sh` appelé depuis CI)
- [ ] Copier `drox-tui-*-linux-x64.tar.gz` + `SHA256SUMS-linux.txt` vers `releases/v<version>/` sur OR
- [ ] Asset GitHub Release Linux attaché au tag
- [ ] Doc [BUILD.md](BUILD.md) 2.0.3

### P1 — UX terminal Linux

- [ ] **Racines workspace** : ajouter `$HOME`, éventuellement `/mnt` (WSL) dans `dir_browser::list_roots`
- [ ] **Souris** : documenter terminaux supportés ; tester + corriger si SGR mal négocié
- [ ] **Presse-papiers** : message i18n si ni `wl-copy` ni `xclip` (déjà partiellement géré)
- [ ] **Collage image** : chemin fichier uniquement sur Linux (déjà) ; doc composer help par plateforme
- [ ] **Raccourcis** : vérifier Meta/Ctrl sur layouts FR Linux (pas seulement AltGr Windows)

### P2 — Qualité & CI

- [ ] Job CI `cargo test -p drox-tui` sur `ubuntu-latest`
- [ ] Job CI build release Linux (artefact optionnel)
- [ ] Section checklist QA Linux (Konsole, GNOME Terminal, Alacritty, WSL2)

### P3 — Améliorations optionnelles (si temps)

- [ ] Collage image clipboard via `wl-paste` / `xclip -t image/png` (best effort)
- [ ] `.desktop` + icône PNG dans archive Linux (menu applications)
- [ ] Détection auto chemin binaire pour `/update install`

---

## Packaging cible OR

```text
releases/v2.0.3/
├── drox-tui-2.0.3-windows-x64-setup.exe
├── drox-tui-2.0.3-linux-x64.tar.gz
├── SHA256SUMS-windows.txt
├── SHA256SUMS-linux.txt
├── RELEASE_NOTES.md
└── (racine repo) releases/latest.json
```

Archive Linux :

```text
drox-tui-2.0.3-linux-x64/
├── drox-tui
├── install.sh
├── LICENSE
├── README-INSTALL.txt
└── VERSION
```

Option : inclure `packaging/assets/drox.png` pour intégrateurs `.desktop`.

---

## Fichiers à modifier

| Fichier | Changement |
|---|---|
| `packaging/publish-or.ps1` | Pipeline Linux + `latest.json` |
| `packaging/build-and-pack-linux.sh` | Copie assets optionnels |
| `engine/dir_browser.rs` | Roots `$HOME`, WSL |
| `engine/image_paste.rs` | Linux clipboard (P3) |
| `terminal/keys.rs` | Tests / ajustements Meta Linux |
| `i18n/*` | Messages plateforme |
| `docs/2.0.3/BUILD.md` | Instructions Linux |

---

## Tests manuels Linux

| Scénario | Attendu |
|---|---|
| `drox-tui --workspace .` | Boot splash, thème Drox, pas de panic |
| `/server` + Ollama local | Connexion OK |
| Souris scroll fil | Défilement si terminal SGR OK |
| `/copy` + `wl-copy` installé | Copie OK |
| `/workspace` | Navigation depuis `$HOME` |
| `/settings` | Modale FR/EN |
| `/update check` | Message clair (après implémentation M2) |

---

## Critères d'acceptation

- [ ] Archive Linux publiée sur OR pour 2.0.3.
- [ ] README installation Linux vérifié sur une machine réelle ou CI.
- [ ] Aucune régression Windows (`cargo test -p drox-tui` + build installateur).
- [ ] Écarts documentés (ex. collage image presse-papiers) dans RELEASE_NOTES.
