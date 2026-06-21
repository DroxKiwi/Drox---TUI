# Animation de lancement Drox — TUI → fork VS Code

Ce dossier contient la **spécification** et le **code de portage** de l’animation splash « DROX » (halo phosphore + logo ASCII), aujourd’hui implémentée dans le TUI :

```54:68:drox/crates/drox-tui/src/ui/boot_splash.rs
pub fn play<B: ratatui::backend::Backend>(
    term: &mut Terminal<B>,
    palette: &ThemePalette,
) -> std::io::Result<()> {
    // 52 frames × 50 ms ≈ 2,6 s
```

L’objectif est de retrouver la **même signature visuelle** dans le fork VS Code Drox au démarrage de la fenêtre (titre produit **Drox**, pas seulement « Code »).

---

## Contenu du dossier

| Fichier | Rôle |
|---|---|
| [drox-logo.ascii.txt](drox-logo.ascii.txt) | Logo block ASCII (6 lignes) |
| [drox-phosphor-theme.json](drox-phosphor-theme.json) | Couleurs + timing (miroir `boot_splash.rs`) |
| [drox-splash-spec.ts](drox-splash-spec.ts) | Implémentation DOM réutilisable dans l’IDE |
| [VSCODE-FORK.md](VSCODE-FORK.md) | Où brancher dans un fork VS Code |
| [extract-splash-from-ide.ps1](extract-splash-from-ide.ps1) | Script pour localiser le splash existant dans le dépôt IDE |

---

## Comportement cible (aligné TUI)

| Phase | Durée relative | Effet |
|---|---|---|
| Apparition | 0 → 48 % | Dissolve entrant, halo radial vert |
| Palier | 48 → 68 % | Logo plein phosphore, tagline |
| Sortie | 68 → 100 % | Fade out, suppression overlay |

- **Durée totale** : ~2,6 s (`52 × 50 ms`)
- **Taglines** : « Initialisation… » puis « Agent local · terminal » (i18n à brancher comme le TUI)
- **Compact** : fenêtre étroite → texte `DROX` au lieu du block ASCII

---

## Intégration rapide (fork VS Code)

### Option A — Workbench part (recommandé)

1. Copier `drox-splash-spec.ts` vers `src/vs/workbench/browser/parts/droxSplash/` (chemin indicatif).
2. Au `Workbench.startup()` (ou équivalent), injecter un `div#drox-splash-root` sur `document.body`.
3. Appeler `createDroxSplashController(root)`.
4. Retirer le splash natif VS Code si présent (voir [VSCODE-FORK.md](VSCODE-FORK.md)).

### Option B — Extension intégrée

Extension `drox-splash` embarquée dans le produit :

```typescript
import { createDroxSplashController } from './drox-splash-spec';

export function activate() {
  const root = document.createElement('div');
  document.body.appendChild(root);
  createDroxSplashController(root);
}
```

Activer en `extensionKind: ui` + `"*": ["onStartupFinished"]` **ou** plus tôt via contribution workbench si vous contrôlez le fork.

### Option C — Webview plein écran

Moins idéal (latence) ; préférer DOM direct comme le spec TS.

---

## product.json (branding)

Dans le fork, vérifier :

```json
{
  "nameShort": "Drox",
  "nameLong": "Drox",
  "applicationName": "drox",
  "windowTitle": "Drox",
  "welcomePage": "…"
}
```

Le splash affiche le **logo ASCII DROX** ; le titre fenêtre reste géré par `product.json`.

---

## Extraction depuis l’IDE existant

Si le fork a déjà une animation, lancez :

```powershell
.\docs\animation-start\extract-splash-from-ide.ps1 -IdeRepo "C:\chemin\vers\Drox-IDE"
```

Le script liste les fichiers contenant `splash`, `gettingStarted`, `startup`, `welcome` et copie un rapport dans `docs/animation-start/reports/`.

---

## Synchronisation TUI ↔ IDE

Quand vous modifiez `boot_splash.rs` :

1. Mettre à jour `drox-phosphor-theme.json` (couleurs / timing)
2. Mettre à jour `drox-splash-spec.ts` (même constantes)
3. Noter le changement dans `docs/2.0.4/CHECKLIST.md`

---

## Références

- Charte : [CHARTE-GRAPHIQUE.md](../2.0.2/CHARTE-GRAPHIQUE.md) § Boot / connexion
- Design system : [THEME.md](../THEME.md)
- Ligne 2.0.4 : [README.md](../2.0.4/README.md)
