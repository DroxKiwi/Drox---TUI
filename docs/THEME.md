# Drox Design System — Thème *Phosphor Terminal*

> **Document normatif portable** — réutilisable sur tout projet (TUI, web, desktop, docs).
> Version du système : **1.0.0** · Première implémentation : Drox TUI 2.0.2

---

## Table des matières

1. [Identité & vision](#1-identité--vision)
2. [Principes de design](#2-principes-de-design)
3. [Système de couleurs](#3-système-de-couleurs)
4. [Rôles sémantiques](#4-rôles-sémantiques)
5. [Typographie](#5-typographie)
6. [Espacement & layout](#6-espacement--layout)
7. [Composants UI](#7-composants-ui)
8. [Motion & animation](#8-motion--animation)
9. [Accessibilité](#9-accessibilité)
10. [Implémentation multi-plateforme](#10-implémentation-multi-plateforme)
11. [Variantes de thème](#11-variantes-de-thème)
12. [Règles Do / Don't](#12-règles-do--dont)
13. [Versioning & migration](#13-versioning--migration)

---

## 1. Identité & vision

### Mot-clé

**Phosphor terminal** — terminal militaire / scientifique des années 1980, réinterprété avec la lisibilité d'un HUD de jeu moderne.

### Références moodboard

| Source | Retenir | Éviter |
|---|---|---|
| **Terminaux CRT IBM 3270** | Vert phosphore sur fond noir, scanlines subtiles | Flicker agressif, illisibilité |
| **Quake 2 HUD** | Contraste net, monospace, sensation opérationnelle | Surcharge visuelle, textures bruyantes |
| **Fallout Pip-Boy** | Cadres utilitaires, vert + ambre parcimonieux | Parodie excessive, humour visuel |
| **Alien MU-TH-UR** | Typographie machine, ton froid et fonctionnel | Horror gimmicks |

### Ton produit

- **Utilitaire** : chaque pixel sert la lecture ou l'action.
- **Diégétique** : l'interface se comporte comme un équipement in-universe (terminal embarqué, console de contrôle).
- **Sobre** : pas de cyberpunk rainbow, pas de Material Design flat coloré.

---

## 2. Principes de design

| # | Principe | Application |
|---|---|---|
| P1 | **Fond toujours sombre** | Jamais de blanc plein en thème Drox principal |
| P2 | **Une couleur d'action** | Le vert phosphore porte l'interaction ; l'ambre = alerte uniquement |
| P3 | **Contraste d'abord** | Ratio ≥ 4.5:1 texte/fond (WCAG AA) |
| P4 | **Monospace partout** | Alignement colonnes, esthétique machine |
| P5 | **ASCII first** | Icônes `[+]`, `[>]`, `[!]` — pas d'emoji UI par défaut |
| P6 | **Parité clavier / souris** | Même widget focusable quel que soit le périphérique |
| P7 | **Animations courtes** | 80–300 ms, désactivables |

---

## 3. Système de couleurs

### 3.1 Tokens primitifs (truecolor)

Ces tokens sont la **source de vérité**. Toute plateforme dérive ses valeurs à partir d'ici.

| Token | Rôle | HEX | RGB | Notes |
|---|---|---|---|---|
| `bg-deep` | Fond racine | `#0a0e0a` | `10, 14, 10` | Noir légèrement verdâtre |
| `bg-panel` | Panneaux, modales | `#0f1410` | `15, 20, 16` | +1 niveau profondeur |
| `bg-elevated` | Header, composer | `#141a14` | `20, 26, 20` | Zone active / surélevée |
| `phosphor-primary` | Texte principal | `#33ff66` | `51, 255, 102` | Vert phosphore lisible |
| `phosphor-dim` | Texte secondaire | `#1a9933` | `26, 153, 51` | Labels, métadonnées |
| `phosphor-bright` | Focus, sélection | `#66ff99` | `102, 255, 153` | Surbrillance |
| `phosphor-glow` | Pulse, curseur | `#00ff41` | `0, 255, 65` | Indicateurs live |
| `amber-alert` | Avertissements | `#ffb000` | `255, 176, 0` | Pip-Boy warning |
| `red-critical` | Erreurs, deny | `#ff3333` | `255, 51, 51` | Permissions refusées |
| `scanline` | Overlay CRT (opt.) | `#33ff6620` | — | ~12 % opacité vert |

### 3.2 Variables CSS (web / docs)

```css
:root {
  /* Backgrounds */
  --drox-bg-deep: #0a0e0a;
  --drox-bg-panel: #0f1410;
  --drox-bg-elevated: #141a14;

  /* Phosphor scale */
  --drox-phosphor-primary: #33ff66;
  --drox-phosphor-dim: #1a9933;
  --drox-phosphor-bright: #66ff99;
  --drox-phosphor-glow: #00ff41;

  /* Semantic */
  --drox-warning: #ffb000;
  --drox-error: #ff3333;

  /* Selection */
  --drox-selection-fg: var(--drox-bg-deep);
  --drox-selection-bg: var(--drox-phosphor-bright);

  /* Typography */
  --drox-font-mono: "IBM Plex Mono", "JetBrains Mono", "Consolas", monospace;
}
```

### 3.3 Fallback ANSI 16 couleurs (`drox-ansi`)

Pour terminaux sans truecolor :

| Token | ANSI approx. |
|---|---|
| `bg-deep` | Black |
| `phosphor-primary` | Green |
| `phosphor-dim` | DarkGreen |
| `phosphor-bright` | LightGreen |
| `amber-alert` | Yellow |
| `red-critical` | Red |
| `selection-fg` | Black |
| `selection-bg` | LightGreen |

---

## 4. Rôles sémantiques

Les **tokens primitifs** ne sont pas utilisés directement dans les composants. On passe par des **rôles sémantiques** :

| Rôle sémantique | Token(s) source | Usage |
|---|---|---|
| `background` | `bg-deep` | Fond application |
| `surface` | `bg-panel` | Modales, panneaux flottants |
| `surface-elevated` | `bg-elevated` | Header, composer, zones actives |
| `text-primary` | `phosphor-primary` | Corps, titres, contenu principal |
| `text-muted` | `phosphor-dim` | Placeholders, hints, labels |
| `accent` | `phosphor-primary` | Liens, titres forts, icônes actives |
| `accent-bright` | `phosphor-bright` | Focus clavier, sélection liste |
| `accent-glow` | `phosphor-glow` | Bordure composer actif, pulse |
| `border-active` | `phosphor-bright` | Bordures focus |
| `border-inactive` | `phosphor-dim` | Bordures repos |
| `warning` | `amber-alert` | Mode plan, tests en cours, bash mode |
| `error` | `red-critical` | Échec connexion, erreurs |
| `selection-fg` | `bg-deep` | Texte sur fond sélectionné |
| `selection-bg` | `phosphor-bright` | Fond item sélectionné / bouton focus |

### Mapping Ratatui (Drox TUI)

Implémentation : `drox/crates/drox-tui/src/ui/theme.rs`

| Rôle sémantique | Champ `ThemePalette` |
|---|---|
| `background` | `bg` |
| `surface` | `bg_panel` |
| `surface-elevated` | `bg_elevated` |
| `text-primary` | `text` |
| `text-muted` | `text_muted` |
| `accent` | `header_primary` |
| `accent-bright` | `accent_bright` |
| `accent-glow` | `accent_glow` |
| `border-active` | `border` |
| `border-inactive` | `border_inactive` |
| `warning` | `warning` |
| `error` | `error` |
| `selection` | `selection_fg` + `selection_bg` |

---

## 5. Typographie

| Propriété | Valeur |
|---|---|
| **Famille** | Monospace obligatoire |
| **Recommandées** | IBM Plex Mono, JetBrains Mono, Consolas, Cascadia Mono |
| **Taille TUI** | 1 cellule = 1 caractère ; pas de scaling interne |
| **Titres modales** | MAJUSCULES ou small-caps simulés (`CONNEXION IA`) |
| **Poids** | Regular par défaut ; **Bold** pour titres et sélection |
| **Italique** | Placeholders composer uniquement |
| **Interlignage** | 1 ligne vide entre sections de modale |

### Hiérarchie textuelle

| Niveau | Style | Exemple |
|---|---|---|
| H1 modal | Bold + `accent` | `CONNEXION SERVEUR IA` |
| H2 étape | `text-muted` | `Etape 2/3 — Moteur d'inference` |
| Body | `text-primary` | Contenu fil assistant |
| Caption | `text-muted` | Hints clavier footer |
| Code | `text-primary` sur `surface` | Blocs markdown |

---

## 6. Espacement & layout

### Grille TUI (cellules)

| Token | Valeur | Usage |
|---|---|---|
| `pad-xs` | 0 | Listes denses |
| `pad-sm` | 1 cellule | Padding interne modale minimum |
| `pad-md` | 2 cellules | Marge popup / bord écran |
| `gap-section` | 1 ligne vide | Entre groupes de champs |

### Structure écran type

```text
┌─ DROX TUI ─ v2.0.2 ─ model ─ mode ─────────────────────────────┐  ← header (3 lignes)
│ workspace · statut connexion                                    │
├─────────────────────────────────────────────────────────────────┤
│ notices (optionnel)                                             │
├─────────────────────────────────────────────────────────────────┤
│ fil assistant (flex, min 4 lignes)                              │
├─────────────────────────────────────────────────────────────────┤
│ panneaux todos / cours / MCP (optionnels, masqués si petit)     │
├─────────────────────────────────────────────────────────────────┤
│ composer (5 lignes)                                             │
├─────────────────────────────────────────────────────────────────┤
│ status line (1 ligne)                                           │
└─────────────────────────────────────────────────────────────────┘
```

### Modales

- Centrées, largeur max **76** cellules, hauteur max **28** lignes
- Fond `surface`, bordure `border-active`
- Titre cadre : `╭─ TITRE ─╮` ou box-drawing classique `┌─ TITRE ─┐`

---

## 7. Composants UI

### 7.1 Header

| État | Fond | Texte modèle | Bordure |
|---|---|---|---|
| Normal | `surface-elevated` | `accent` bold | `border-inactive` |
| Plan mode | idem | tag `warning` | idem |

### 7.2 Composer

| État | Bordure | Texte |
|---|---|---|
| Actif | `accent-glow` | `text-primary` |
| Bloqué (run) | `border-inactive` | placeholder `text-muted` italic |
| Bash mode | `warning` | `warning` |
| Curseur | reverse video ou block `selection` | — |

### 7.3 Listes & sélection

| État | Style |
|---|---|
| Item normal | `text-muted`, marqueur ` ` |
| Item sélectionné | `selection-fg` sur `selection-bg`, marqueur `>` |
| Hover (souris) | même que sélection |

### 7.4 Champs de saisie

| Partie | Style |
|---|---|
| Label | `text-muted` + `: ` |
| Valeur | `text-primary` |
| Curseur | `selection-fg` sur `selection-bg` |
| Secret | `*` répétés, même curseur |

### 7.5 Boutons (TUI)

Format : ` [ Label ] ` — pas de coins arrondis Unicode.

| État | Style |
|---|---|
| Repos | `text-muted` |
| Focus | `selection-fg` sur `selection-bg` bold |

### 7.6 Messages de statut

| Type | Couleur |
|---|---|
| Info | `text-muted` |
| Succès | `accent` |
| Test en cours | `warning` |
| Erreur | `error` |

### 7.7 Pastille connexion (header)

| État | Symbole | Couleur |
|---|---|---|
| Connecté | `●` | `accent-glow` |
| Partiel / plan | `●` | `warning` |
| Déconnecté | `○` | `error` |

---

## 8. Motion & animation

### Tokens temporels

| Token | Durée | Easing |
|---|---|---|
| `duration-instant` | 0 ms | — |
| `duration-fast` | 80 ms | linear |
| `duration-normal` | 150 ms | linear |
| `duration-slow` | 300 ms | linear |
| `cursor-blink` | 530 ms | step |

### Catalogue d'effets

| ID | Description | Cible | Désactivable |
|---|---|---|---|
| `cursor-blink` | Curseur block clignotant | Composer | oui |
| `phosphor-pulse` | Bordure alterne primary ↔ glow | Composer actif | oui |
| `modal-in` | Cadre modal dessiné en 4 frames | Modales | oui |
| `handshake` | Barre progression connexion | `/server` test | oui |
| `stream-glow` | Dernière ligne assistant pulse | Fil streaming | oui |
| `scanline-overlay` | Lignes CRT 8 % opacité | Fond global | oui |

**Règle** : animations ≤ 60 fps, jamais bloquantes, toggle `/settings animations: off`.

---

## 9. Accessibilité

| Critère | Cible | Notes |
|---|---|---|
| Contraste texte/fond | ≥ 4.5:1 | `phosphor-primary` / `bg-deep` ≈ 11:1 |
| Focus visible | Obligatoire | `accent-bright` sur tout widget interactif |
| Pas de couleur seule | Icône + texte | `[!]` + message, pas seulement rouge |
| Reduced motion | Respecter pref OS | Désactiver pulse / scanline |
| Terminal 16 couleurs | Variante `drox-ansi` | Toujours proposer |

---

## 10. Implémentation multi-plateforme

### 10.1 Ratatui / TUI Rust

```rust
// Thème par défaut
let palette = TuiThemeSetting::Drox.palette();

// Rendu bloc
Block::default()
    .style(Style::default().fg(palette.border).bg(palette.bg_panel));

// Sélection liste
Style::default()
    .fg(palette.selection_fg)
    .bg(palette.accent_bright)
    .add_modifier(Modifier::BOLD);
```

Constantes exportées : `crate::ui::theme::drox::{BG_DEEP, PHOSPHOR_PRIMARY, ...}`

### 10.2 CSS / Web

Utiliser les variables `:root` de la section 3.2. Exemple composant :

```css
.drox-modal {
  background: var(--drox-bg-panel);
  border: 1px solid var(--drox-phosphor-bright);
  color: var(--drox-phosphor-primary);
  font-family: var(--drox-font-mono);
}

.drox-modal__title {
  text-transform: uppercase;
  font-weight: 700;
  color: var(--drox-phosphor-primary);
}

.drox-list-item--selected {
  background: var(--drox-selection-bg);
  color: var(--drox-selection-fg);
}
```

### 10.3 Terminal ANSI (script shell)

```bash
# Drox-ansi prompt minimal
PS1='\[\033[30;42m\] DROX \[\033[0m\]\[\033[32m\]\w\[\033[0m\] \$ '
```

### 10.4 Figma / design tools

Créer un **style library** avec les 10 tokens primitifs + 12 rôles sémantiques. Nommer les styles `drox/phosphor-primary`, `drox/bg-panel`, etc.

### 10.5 Accent session (`/color`)

En thème Drox strict, l'accent utilisateur ne recolore **pas** le phosphore principal — il affecte uniquement les **tags / badges** (`mode_tag`). Les thèmes legacy (`dark`, `light`, …) conservent le comportement complet.

---

## 11. Variantes de thème

| ID | Slug | Description | Défaut |
|---|---|---|---|
| Drox | `drox` | Truecolor phosphore | **oui** (2.0.2+) |
| Drox ANSI | `drox-ansi` | 16 couleurs | non |
| Dark | `dark` | Cyan/yellow legacy | non |
| Light | `light` | Thème clair legacy | non |
| Dark ANSI | `dark-ansi` | Legacy 16c sombre | non |
| Light ANSI | `light-ansi` | Legacy 16c clair | non |
| Dim | `dim` | Gris-bleu atténué | non |

Sélection : `/theme <slug>` ou picker interactif.

---

## 12. Règles Do / Don't

### Do

- Utiliser les **rôles sémantiques**, pas les hex en dur dans les composants
- Garder l'ambre pour **avertissements** uniquement
- Titres modales en **MAJUSCULES**
- Prévoir un **fallback ANSI** pour chaque écran critique
- Tester sur **Windows Terminal truecolor** et **cmd 16 couleurs**

### Don't

- Blanc pur `#ffffff` en fond
- Arc-en-ciel cyberpunk, dégradés multicolores
- Emoji comme icônes UI par défaut
- Animations > 300 ms ou bloquantes
- Recolorer tout le chrome avec `/color` en mode Drox strict

---

## 13. Versioning & migration

| Version | Changements |
|---|---|
| **1.0.0** | Tokens primitifs, rôles sémantiques, variantes `drox` + `drox-ansi`, défaut produit 2.0.2 |

### Migration depuis thème `dark` (2.0.1)

- Les utilisateurs existants conservent `theme: "dark"` dans leurs prefs
- Nouvelle install → `theme: "drox"` par défaut
- Pas de migration forcée

### Extension future

Pour ajouter un token :

1. Définir le primitif dans ce document (section 3)
2. Mapper un rôle sémantique (section 4)
3. Ajouter le champ à `ThemePalette` + toutes les variantes
4. Bumper version mineure du design system

---

## Références

| Document | Lien |
|---|---|
| Charte produit 2.0.2 | [docs/2.0.2/CHARTE-GRAPHIQUE.md](2.0.2/CHARTE-GRAPHIQUE.md) |
| Implémentation Rust | `drox/crates/drox-tui/src/ui/theme.rs` |
| Checklist QA visuelle | [docs/2.0.2/CHECKLIST.md](2.0.2/CHECKLIST.md) |

---

*Ce document est autonome et portable. Pour l'adopter sur un autre projet, copier les sections 3–4 (couleurs + sémantique) et adapter la section 10 à votre stack.*
