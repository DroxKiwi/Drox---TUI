# F03 — Context manifest (connaissance LLM réelle)

**Statut** : spec · **Ligne** : 2.0.6 M2  
**Consommateurs** : panneau carte (F05), panel IDE « contexte actuel »

---

## Objectif

Représenter ce que le modèle a **réellement en contexte** au tour LLM courant — pas seulement les fichiers explorés sur disque.

Distinction critique :

| Concept | Source |
|---|---|
| **Exploré** | `WorkspaceMapStore`, tools `glob` / `file_read` |
| **En contexte** | Messages assemblés avant appel LLM (non compactés) |
| **Évincé** | Snippé / résumé par `ContextSnip` ou `ContextCompacted` |
| **Structure seule** | Présent dans `[Workspace map]` prompt, contenu absent |

---

## États nœud

| État | Glyphe suggéré | Couleur |
|---|---|---|
| `in_context` | `●` | beat / accent fort |
| `evicted` | `○` | muted |
| `mapped_only` | `◇` | border |
| `modified_run` | `▲` | warning / diff |

---

## Modèle de données (`ContextManifest`)

```json
{
  "turn_index": 7,
  "token_estimate": 42000,
  "token_budget": 128000,
  "nodes": [
    {
      "path": "src/lib.rs",
      "state": "in_context",
      "beat_ids": ["A12"],
      "source": "tool_result:file_read",
      "bytes_in_prompt": 8192
    },
    {
      "path": "docs/old.md",
      "state": "evicted",
      "beat_ids": ["A5"],
      "evicted_by": "context_snip",
      "summary_only": true
    }
  ]
}
```

Recalculé **après chaque tour LLM** (hook observe, pas recalcul UI).

---

## Implémentation cible

| Composant | Rôle |
|---|---|
| `drox-engine` | Hooks existants : `ContextSnip`, compaction — **pas** de logique UI |
| `drox-observe` | `ContextManifestBuilder` — lit historique messages + policy |
| `drox-tui` | Rendu arbre F05 depuis snapshot |

**Ne pas** confondre avec `workspace-map.json` persisté — le manifest est **éphémère par tour**, la map est **trajectoire longue durée**.

---

## Port IDE

- Webview « Context inspector » : liste + jauge tokens.
- Même JSON via `observe/context_snapshot`.

---

## Risques

| Risque | Mitigation |
|---|---|
| Estimation tokens imprécise | Fourchette + label « ~approx » |
| Perf rebuild manifest | Snapshot incrémental, debounce 100 ms |
| Fuite PII dans debug | Opt-in verbose, jamais dans logs prod |
