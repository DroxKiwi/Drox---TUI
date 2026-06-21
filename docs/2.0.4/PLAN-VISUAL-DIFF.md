# Plan — Diff visuel TUI (ligne 2.0.4)

> **Principe produit** : l’utilisateur doit **voir** les modifications — ajouts, suppressions, fichiers touchés — **dans le TUI**, sans quitter Drox ni deviner via `git status` seul. Le diff n’est pas une fonction interne agent : c’est une **fonctionnalité visible** pour le workspace et pour les changements proposés/appliqués.

> **Constat technique** : la chaîne diff unifié (génération → couleurs → viewer scrollable) existe déjà, mais n’est branchée que partiellement (`file_edit` + touche `e`, modale permission). `/diff` et plusieurs chemins agent restent sous-exploités.

---

## Règles UX (2.0.4)

1. **Tout changement lisible** — patch unifié coloré (`+` / `-` / `@`), pas seulement un résumé.
2. **Pas de cul-de-sac** — si le fil tronque, un chemin évident mène au diff complet (overlay ou `e`).
3. **Même widget partout** — `LinesViewer` + `scroll_overlay` pour workspace git et outils agent.
4. **Workspace d’abord** — `/diff` ouvre le diff visuel par défaut ; `--stat` garde le mode compact.

---

## Inventaire — ce qui existe déjà

### Génération de patch

| Composant | Fichier | Rôle |
|---|---|---|
| `unified_line_diff` | `drox-tools/src/diff_util.rs` | Diff unifié via crate `similar` (contexte 3 lignes) |
| `preview_file_edit_diff` | `drox-tools/src/simple/file_edit.rs` | Preview avant permission |
| `preview_file_write_diff` | `drox-tools/src/simple/file_write.rs` | Idem création fichier |
| `preview_notebook_edit_diff` | `drox-tools/src/simple/notebook_edit.rs` | Cellules notebook |
| Sortie outil | JSON champ `diff` sur `file_edit` / `notebook_edit` | Patch persisté dans le fil |

### Affichage coloré

| Composant | Fichier | Comportement |
|---|---|---|
| `LinesViewerStyle::UnifiedDiff` | `view/lines_viewer.rs` | `+` vert, `-` rouge, `@` magenta, `+++`/`---` cyan |
| `render_diff_line` | idem | Coloration ligne par ligne |
| `append_unified_diff_block` | `view/tool_output.rs` | Max **16 lignes** dans le fil ; hint `e pour parcourir` |
| `PermissionPreviewBody::FileDiff` | `view/permission_preview.rs` | Max **28 lignes** dans modale permission |
| `diff_is_expandable` | `tool_output.rs` | Seuil 16 lignes → touche **`e`** ouvre `ScrollViewer` |

### Navigation (déjà câblée)

| Action | Mécanisme |
|---|---|
| Touche `e` sur outil expandable | `app/state.rs` → `ScrollViewerState::Lines(diff_viewer)` |
| Scroll PgUp/PgDown | `view/scroll_viewer.rs` |
| Overlay plein écran | `widgets/scroll_overlay.rs` |

### Ce qui manque / sous-exploité

| Lacune | Détail |
|---|---|
| **`/diff` minimal** | `engine/diff_cmd.rs` : `git status --short` + `git diff --stat HEAD` seulement ; renvoie vers shell pour le patch |
| **Pas de diff fichier ciblé** | Impossible `/diff src/foo.rs` ou clic sur une ligne `git status` |
| **Pas de side-by-side** | Uniquement unified (style `git diff`) |
| **Pas de numéros de ligne source** | Le viewer affiche le texte du patch, pas les numéros de fichier alignés |
| **Pas de syntaxe dans les hunks** | Lignes diff = texte brut coloré par préfixe |
| **`file_write` proposé** | Preview = contenu brut, pas diff vs fichier vide / existant dans le fil |
| **Pas de diff inter-sessions** | `/rewind` ne montre pas le delta message |

---

## Vision 2.0.4

Un **diff visuel first-class** : l’utilisateur voit les changements du workspace et ceux proposés par l’agent avec la même qualité que les previews permission, sans quitter le TUI.

```text
┌─ /diff ───────────────────────────────────────────────────────┐
│ M  src/app.rs    A  docs/README.md    ??  tmp.log            │
│ ── git diff (unifié) ─────────────────────────────────────  │
│ --- a/src/app.rs                                             │
│ +++ b/src/app.rs                                             │
│ @@ -12,4 +12,5 @@                                            │
│  fn main() {                                                 │
│ -    println!("old");                                        │
│ +    println!("drox");                                       │
│ ──────────────────────────────────────────────────────────── │
│  PgUp/PgDown · o ouvrir fichier · Esc fermer                 │
└──────────────────────────────────────────────────────────────┘
```

---

## Jalons

### M1 — `/diff` visuel + agent lisible (quick win) ✅

- [x] `diff_cmd.rs` : `git diff HEAD` unifié + en-tête `git status --short`
- [x] `/diff` sans arg → **overlay scrollable** (`LinesViewer` coloré)
- [x] `/diff --stat` → ancien résumé texte dans le fil
- [x] Repo non-git / aucun changement → message i18n clair
- [x] i18n FR/EN : palette, statut
- [x] Fil agent : `e` sur tout diff non vide + hint dans le fil

**Fichiers** : `engine/diff_cmd.rs`, `slash.rs`, `app/run.rs`, `view/lines_viewer.rs`, `view/tool_output.rs`, `i18n/*`

### M2 — Diff fichier et navigation ✅

- [x] `/diff <chemin>` → diff du fichier (git ou working tree)
- [x] Liste `git status --short` cliquable (souris déjà active sur modales)
- [x] Raccourci `o` dans le viewer → `file_read` sur le fichier
- [x] `Entrée` / ↑↓ sur la sélection status pour cibler un fichier

### M3 — Enrichissement visuel ✅

- [x] Numéros de ligne dans la marge (parser les en-têtes `@@`)
- [x] Surlignage intra-ligne (`similar` word-diff) pour petits hunks
- [x] Thème : couleurs diff depuis `ThemePalette` (pas hardcodé Green/Red)
- [x] `file_write` proposé : afficher unified diff dans le fil (comme `file_edit`)

### M4 — Diff agent intégré (polish) ✅

- [x] Bandeau « N fichiers modifiés » en fin de run avec `Entrée` → viewer multi-fichier
- [x] Permission modal : lien « voir diff complet » → même overlay que M1 (`e`)
- [x] Tests : `tool_output` run viewer, `permission_preview` full_diff

---

## Architecture cible

```text
diff_cmd / tool_output / permission_preview
        │
        ▼
   DiffSource (enum)
   ├─ GitWorkspace { range, path? }
   ├─ ToolOutput { id, path, patch }
   └─ PermissionPreview { path, before, after }
        │
        ▼
   LinesViewerState (UnifiedDiff)  ──►  ScrollViewerState
        │
        ▼
   scroll_overlay (existant)
```

Nouveau module suggéré : `engine/diff_view.rs` (agrégation sources) — **seulement si** M2+ évite la duplication.

---

## Dépendances

- Crate `similar` déjà dans le workspace (`drox-tools`)
- Pas de dépendance `git2` requise (subprocess `git` comme aujourd’hui)
- Option future : `ratatui` widgets custom pour side-by-side (hors 2.0.4)

---

## QA

- [ ] Repo git avec modifications staged/unstaged
- [ ] Repo non-git → message clair
- [ ] Fichier binaire → pas de crash
- [ ] Diff > 500 lignes → scroll fluide
- [ ] FR + EN

---

## Références

- Viewer existant : ```40:50:drox/crates/drox-tui/src/view/lines_viewer.rs```
- Expand `e` : ```1307:1316:drox/crates/drox-tui/src/app/state.rs```
- `/diff` actuel : ```8:30:drox/crates/drox-tui/src/engine/diff_cmd.rs```
