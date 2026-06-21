# Features 2.0.5 — index des specs

> **Doc produit** de la ligne 2.0.5. L’**externalisation** concerne le **code moteur** (crate `drox-observe`, modules par feature) — pas un dossier doc transversal. Voir [ARCHITECTURE-FEATURES.md](ARCHITECTURE-FEATURES.md).

Chaque fiche décrit le comportement UX + contrat de données ; l’implémentation vit dans les modules Rust correspondants.

---

## Index

| ID | Spec | Jalon | Module code cible (`drox-observe`) |
|---|---|---|---|
| **F01** | [Multi-pane shell](F01-multi-pane-shell.md) | M0 | *(TUI `panes/` — pas observe)* |
| **F02** | [Beat ID & corrélation](F02-beat-id-correlation.md) | M0–M1 | `beat/` |
| **F03** | [Context manifest](F03-context-manifest.md) | M2 | `manifest/` |
| **F04** | [Changements + diff](F04-run-changes-panel.md) | M1 | `changes/` |
| **F05** | [Carte workspace](F05-workspace-map-view.md) | M2 | `map_view/` |

---

## Port IDE (effet de bord voulu)

Les types `serde` produits par `drox-observe` sont **réutilisables** par le fork VS Code (même JSON, UI native). Les fiches ci-dessus documentent le contrat ; le port IDE ne duplique pas la doc dans un autre dossier.

Référence fork : [`docs/animation-start/VSCODE-FORK.md`](../animation-start/VSCODE-FORK.md).
