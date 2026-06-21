# Plan — Diff inline dans le fil + split pane (ligne 2.0.5)

> **Intention produit** (clarification post-2.0.4) : le diff doit être **visible dans le fil de discussion**, pas uniquement via overlay `/diff` ou touche `e`. Référence UX : **Claude Code TUI** — patch coloré sous l’outil, clic pour agrandir.

**Statut** : plan  
**Prédécesseur technique** : [`docs/2.0.4/PLAN-VISUAL-DIFF.md`](../2.0.4/PLAN-VISUAL-DIFF.md)

---

## Problème actuel (fin 2.0.4)

| Ce qui marche | Ce qui manque |
|---|---|
| Diff tronqué (16 lignes) dans le fil outil | Pas assez « conversationnel » — ressemble à un log technique |
| Overlay plein écran (`e`, `/diff`, bandeau run) | Pas de **split** fil + fichier |
| Clic status / `o` sur viewer git | Pas de lien depuis le **bloc diff inline** du fil agent |

L’utilisateur veut **lire la discussion et le code modifié côte à côte**, sans perdre le contexte du run.

---

## Cible UX

### Fil agent (mode normal)

```
▸ file_write (tu_…) ✓ RULES.md
    --- a/RULES.md
    +++ b/RULES.md
    @@ -1,3 +1,12 @@
    +# Règles Git
    +…
    … +9 lignes · Entrée pour ouvrir
```

- Patch **unifié coloré** (`+` vert, `-` rouge, `@` magenta) — réutiliser `render_diff_line`.
- Bloc **cliquable / focus** (souris + clavier).
- Hint : `Entrée` ou clic → mode split.

### Mode split (activation)

```
┌─────────────────────────────┬─────────────────────────────┐
│ Fil (scroll indépendant)    │ RULES.md — diff complet     │
│ … messages, tools …         │ (scroll indépendant)        │
│                             │ numéros de ligne, word-diff │
└─────────────────────────────┴─────────────────────────────┘
 Esc · q  fermer split · Tab basculer focus
```

- **Ratio** par défaut 50/50 (configurable plus tard).
- Panneau droit = `LinesViewerState` existant, pas un nouveau renderer.
- Panneau gauche = fil actuel (composer en bas inchangé ou réduit — à trancher M1).

### Déclencheurs d’ouverture split

| Source | Action |
|---|---|
| Clic sur bloc diff inline | Ouvre split sur ce fichier |
| `Entrée` sur ligne diff focus | Idem |
| Bandeau fin de run (2.0.4) | Option : premier fichier ou liste dans panneau droit |
| `/diff <fichier>` | Peut rester overlay **ou** réutiliser split (M3) |

---

## Architecture proposée

### État TUI

```rust
// app/state.rs (concept)
pub struct SplitDiffView {
    pub file_path: String,
    pub viewer: LinesViewerState,
    pub focus: SplitFocus, // LeftFeed | RightDiff
    pub split_ratio: u8,   // 30..70
}
```

- `AppState.split_diff: Option<SplitDiffView>`
- Exclusif avec `scroll_viewer` overlay **ou** migration progressive : split remplace overlay pour les diffs agent.

### Rendu layout

```
widgets/
  split_layout.rs    # Constraint HORIZONTAL, deux Rect
  feed_panel.rs        # fil existant, scroll propre
  diff_panel.rs        # wrapper LinesViewer + header chemin
```

Point d’entrée : `ui/layout.rs` ou `widgets/root.rs` — branche si `split_diff.is_some()`.

### Données diff

| Source | Déjà disponible |
|---|---|
| `ToolFinish` `file_write` / `file_edit` | JSON `diff` |
| `RunFileChange` (fin de run) | `tool_output::RunFileChange` |
| Workspace `/diff` | `engine/diff_cmd.rs` |

Pas de nouvelle génération patch — **brancher l’affichage**.

### Interaction souris

Réutiliser l’infra 2.0.4 (`app/mouse.rs`) :

- Hit-test sur `LogEntry::ToolFinish` avec diff expandable.
- `MouseEventKind::Down` → `open_split_diff(path, patch)`.

---

## Jalons

### M1 — Diff inline visible (fil seul)

- [ ] Toujours afficher le bloc coloré sous `file_write` / `file_edit` / `notebook_edit` (pas seulement si expandable > 16 lignes).
- [ ] Style « carte » légère (bordure theme `diff_*`).
- [ ] Hint clavier unifié : `Entrée` ouvre le viewer (overlay en interim si split pas prêt).

### M2 — Split pane

- [ ] `SplitDiffView` + layout 50/50.
- [ ] Scroll indépendant (molette / PgUp/PgDn selon focus).
- [ ] `Esc` ferme le split, retour fil pleine largeur.
- [ ] Clic bloc diff → split.

### M3 — Navigation multi-fichiers

- [ ] Fin de run : liste fichiers dans panneau droit (`j`/`k`) si plusieurs `RunFileChange`.
- [ ] `/diff` sans arg : split avec liste git (réutiliser `git_nav` 2.0.4).
- [ ] i18n FR/EN hints split.

### M4 — Finitions

- [ ] Mémoriser ratio split dans `tui-preferences.json` (optionnel).
- [ ] Tests snapshot rendu diff inline + hit-test souris.
- [ ] Doc README utilisateur (capture fil + split).

---

## Risques et garde-fous

| Risque | Mitigation |
|---|---|
| Terminal étroit (< 120 cols) | Split minimum 80 cols ; sinon fallback overlay plein écran |
| Régression overlay `/diff` | Garder `scroll_viewer` ; split est un **mode additionnel** en M2 |
| Perf fil long | Pas re-render diff complet à chaque frame — cache `LogRenderCache` + panneau droit isolé |
| Conflit avec modales | Split désactivé si `AppPhase::Prompt` ou modale ouverte |

---

## Critères d’acceptation

1. Après un `file_write`, le fil montre un patch coloré **sans** appuyer sur `e`.
2. Clic ou `Entrée` ouvre fil + diff côte à côte.
3. `Esc` restaure le fil plein écran sans perdre l’historique.
4. Aucune régression sur permission preview diff ni bandeau run 2.0.4.

---

## Références code (point de départ)

| Zone | Fichier |
|---|---|
| Troncature fil | `view/tool_output.rs` |
| Rendu lignes diff | `view/diff_render.rs`, `view/lines_viewer.rs` |
| Layout principal | `ui/layout.rs`, `widgets/` |
| Souris | `app/mouse.rs` |
| Plan overlay 2.0.4 | `docs/2.0.4/PLAN-VISUAL-DIFF.md` |
