# Bug — réponses assistant en double (2.0.4)

**Statut** : corrigé sur branche `2.0.4`  
**Sévérité** : UX — fil de discussion illisible, impression de « double réponse »  
**Introduit / visible** : après stabilisation des phases agent + nudges moteur (cas fréquent GLM / tours sans `[phase: done]`)

---

## Symptôme

Dans le fil TUI, la même réponse assistant apparaît **deux fois** :

1. Un bloc markdown avec préfixe `◂` (entrée `LogEntry::Assistant`)
2. Un bloc `── [answering] ──` avec un contenu quasi identique

Exemple typique après `file_write` + `todo_write` : l’agent répond, puis le moteur relance un tour de clôture.

---

## Cause racine

Deux mécanismes combinés :

### 1. Routage incorrect du buffer streaming (`apply_agent_event`)

Lors d’un `PhaseEnter` (notamment `[phase: done]`), le texte accumulé en streaming était **toujours** vidé en `LogEntry::Assistant`, alors que le rejeu transcript (`transcript_replay.rs`) le range en `LogEntry::PhaseLine` **sous** le `PhaseOpen { Answering }`.

Résultat live :

| Attendu (replay) | Affiché (bug) |
|---|---|
| `PhaseOpen(Answering)` + `PhaseLine(texte)` | `PhaseOpen(Answering)` + `Assistant(texte)` séparé |

Le fil montrait donc un header `[answering]` orphelin **et** un bloc assistant autonome avec le même texte.

### 2. Nudge moteur « done manquant »

Quand le modèle termine en phase `answering` sans émettre `[phase: done]`, le moteur (`drox-engine/src/agent.rs`) injecte un nudge (`DONE_ONLY_NUDGE_PROMPT`) et relance un tour.

- Si le modèle obéit (marqueur seul) → risque d’un **second header `[answering]`** vide.
- Si le modèle réécrit toute la réponse (ancien `NUDGE_PROMPT` générique) → **contenu dupliqué** malgré le nudge.

Le commentaire moteur documente déjà ce risque (L1094–1098 de `agent.rs`).

---

## Correctif TUI (2.0.4)

Fichier : `drox/crates/drox-tui/src/engine/bootstrap.rs`

| Changement | Détail |
|---|---|
| `stream_flush_kind()` | Si un `PhaseOpen` est actif → flush en `PhaseLine`, sinon `Assistant` |
| `PhaseEnter(Done)` | Flush en `PhaseLine`, **pas** de header `[done]` (aligné replay) |
| `PhaseEnter(Answering)` vide | Ignore le second header si un bloc `answering` est déjà ouvert (nudge done-only) |
| `Stop` / `ToolStart` / `PhaseClose` | Même règle de routage |

Tests : `stream_flush_tests` dans `bootstrap.rs`.

---

## Non-régression / limites

- **Conversation sans phase** (`TextDelta` → `Stop` sans `PhaseOpen`) : reste un `LogEntry::Assistant` unique.
- **Réécriture complète** après nudge générique : le moteur peut encore demander une seconde réponse ; le correctif TUI évite la duplication **structurelle** (Assistant + PhaseLine), pas une réécriture volontaire du LLM.
- **Rehydratation transcript** : inchangée ; live et replay convergent.

---

## Vérification manuelle

1. Lancer un run avec mutation (`file_write`) + `todo_write`.
2. Laisser le modèle répondre en `[phase: answering]` (sans `[phase: done]` si possible).
3. Vérifier **une seule** zone de texte sous `── [answering] ──`, sans bloc `◂` redondant.

---

## Références

- Handler événements : `engine/bootstrap.rs` → `apply_agent_event`
- Replay transcript : `view/transcript_replay.rs` → `push_assistant_text`
- Nudge done-only : `drox-engine/src/agent.rs` → `DONE_ONLY_NUDGE_PROMPT`
