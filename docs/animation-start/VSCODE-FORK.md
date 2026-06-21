# Intégration splash dans un fork VS Code (Drox IDE)

> Guide pour retrouver et remplacer l’animation de démarrage par la spec Drox (`drox-splash-spec.ts`).

---

## 1. Fichiers typiques à inspecter

| Zone | Chemins habituels (VS Code / fork) |
|---|---|
| Splash natif | `src/vs/workbench/contrib/splash/browser/splash.contribution.ts` |
| Page d’accueil | `src/vs/workbench/contrib/welcomeGettingStarted/` |
| Titre / branding | `product.json` (racine) |
| Icônes | `resources/win32/code.ico`, `resources/linux/code.png` |
| Démarrage workbench | `src/vs/workbench/browser/workbench.ts` |
| Parts layout | `src/vs/workbench/browser/layout.ts` |

Les chemins exacts varient selon la version du fork (1.85–1.99+). Utilisez le script `extract-splash-from-ide.ps1`.

---

## 2. Désactiver le splash Microsoft (si présent)

Rechercher dans le fork :

```text
workbench.contrib.splash
gettingStarted
StartupPage
```

Options :

- Retirer l’enregistrement du contribution point splash
- Ou conditionner : `if (product.nameShort === 'Drox') { … }`

---

## 3. Point d’accroche recommandé

Dans `Workbench` startup (pseudo-code) :

```typescript
import { createDroxSplashController } from 'vs/workbench/browser/parts/droxSplash/drox-splash-spec';

// Avant render workbench, après DOM ready :
const splashRoot = document.createElement('div');
splashRoot.id = 'drox-splash-root';
document.body.appendChild(splashRoot);
const splash = createDroxSplashController(splashRoot);

// Optionnel : splash.stop() si l’utilisateur ouvre une session avant la fin
```

Le splash se retire tout seul après ~2,6 s (voir `DEFAULT_CONFIG.frames`).

---

## 4. Build / bundling

- Le fichier TS doit passer par le bundler du workbench (même `tsconfig` que `src/vs/`).
- Pas de dépendance npm externe : le spec est autonome.
- Pour Electron : le splash est du DOM pur (pas de `canvas`).

---

## 5. i18n

Mapper les clés TUI :

| Clé TUI (`i18n`) | FR | EN |
|---|---|---|
| `boot.tagline` | Agent local · terminal | Local agent · terminal |
| `boot.init` | Initialisation… | Initializing… |

Dans l’IDE, utiliser `nls.localize` ou votre couche i18n existante pour remplir `DroxSplashConfig.taglineBoot` / `taglineProduct`.

---

## 6. Test visuel

1. Lancer l’IDE en dev : `./scripts/code.sh` ou `code.bat`
2. Vérifier fond `#030704`, halo vert, logo 6 lignes centré
3. Redimensionner fenêtre < 480 px → bascule `DROX` compact
4. Comparer côte à côte avec `drox-tui` (même charte)

---

## 7. Si le fork utilise déjà une animation « Drox »

1. Exporter les assets (SVG, Lottie, CSS) via `extract-splash-from-ide.ps1`
2. Documenter les écarts vs spec phosphore
3. Décider : remplacer entièrement ou fusionner (logo ASCII + assets existants)

---

## 8. Prochaine étape 2.0.4

- [ ] Chemin exact du dépôt IDE confirmé dans ce doc
- [ ] PR fork : intégration `drox-splash-spec.ts`
- [ ] Capture vidéo TUI vs IDE pour validation charte
