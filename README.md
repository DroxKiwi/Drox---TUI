# Drox TUI

Client **terminal** pour l’agent Drox — **open source**, basé sur le même moteur Rust que [Drox IDE](https://github.com/DroxKiwi/Drox---IDE).

> **Très expérimental.** Le TUI existe surtout pour **m’aider à construire l’IDE** : dogfood du moteur, boucle agent, UX fil / outils, sans le poids d’Electron. Je **reprendrai potentiellement** son développement plus tard, quand l’IDE sera plus stable. En attendant, attends-toi à des trous, des cassures et un rythme de polish secondaire.

## But du projet

Je veux un assistant agent **dans le terminal**, entièrement open source, pour travailler sur de vrais projets sans dépendre d’un produit fermé ni d’un cloud imposé. Le TUI est la peau la plus simple pour s’approprier le moteur — et surtout un **banc d’essai** au service de l’IDE.

Principes non négociables :

- **Aucune télémétrie** Drox / KDDS.
- **Aucun appel imposé** vers des clouds tiers pour faire tourner l’agent.
- Sessions, transcripts et réglages **sur ta machine** (`~/.drox/`, `<workspace>/.drox/`).
- Le LLM, c’est **le tien** : [Ollama](https://ollama.com/) en local, ou un endpoint OpenAI-compatible que **tu** configures (`/server`, `Ctrl+Shift+L`).

Exception réseau **opt-in** uniquement : la vérif de version (`/update on`) lit [`releases/latest.json`](releases/latest.json) sur **ce** dépôt.

Tu gardes le confort d’un agent qui lit, planifie et modifie le code — avec une souveraineté réelle sur la pile.

| | |
|---|---|
| **Licence** | MIT — [`LICENSE`](LICENSE) · [`NOTICE.md`](NOTICE.md) |
| **Version** | **2.0.6** · moteur agent **1.5.0** (`tui_mono`) |
| **Binaires** | [Releases](https://github.com/DroxKiwi/Drox---TUI/releases) |
| **Doc moteur** | [`docs/engine/`](docs/engine/README.md) |
| **Sœur** | [Drox IDE](https://github.com/DroxKiwi/Drox---IDE) |
| **État** | **Très expérimental** — banc d’essai pour l’IDE ; polish TUI en pause relative |

```bash
# Sources (Rust ≥ 1.85)
cd drox && cargo build -p drox-tui --release
./target/release/drox-tui   # Windows : target\release\drox-tui.exe
```

---

## Historique

Les dates s’appuient sur l’historique git de ce dépôt.

### Avant / laboratoire

J’ai d’abord fait vivre le **moteur agent en Rust** (boucle LLM, outils, permissions) avec ce client terminal pour valider le comportement **hors IDE**. L’idée : un cerveau réutilisable — le TUI comme première peau, l’IDE comme seconde.

### Juin 2026 — ligne 2.0.x

| Date | Étape |
|------|--------|
| **juin** | **2.0.1 – 2.0.2** : packaging Windows, i18n, logo installateur. |
| **21 juin** | **2.0.3** : `/update` opt-in, check semver, bandeau MAJ, install SHA256, Linux. |
| **21 juin** | **2.0.4** : **diff visuel** (`/diff`, overlay, navigation, thème). |
| **21 juin – 1er juil.** | **2.0.5** : **connexions LLM** self-hosted et cloud, pipeline publish Win/Linux. |
| **28 sept.** | **2.0.6** : `/update` pointe vers **ce** dépôt (fin du canal OR). |

### Septembre 2026 — ouverture totale

| Date | Étape |
|------|--------|
| **28 septembre** | Décision produit : **dévoiler complètement** le code. Licence MIT, `docs/engine/`, Releases sur **ce** dépôt. |

Aujourd’hui je pense ce dépôt pour la communauté : cloner, forker, lire, contribuer — ou simplement télécharger l’installeur.

---

## Pourquoi cet outil existe

Des outils comme **Cursor** ont montré qu’un agent change le quotidien. Ils restent des produits d’entreprise : compte, cloud, règles du vendeur.

Drox TUI est né de mon besoin de **retrouver un confort comparable** — certes avec moins de puissance brute — **sans dépendre d’une société** pour le cœur local. Le terminal est l’endroit le plus simple pour dogfooder le moteur **au service de l’IDE** : avancer vite sur la boucle agent, puis reporter ce qui tient dans le workbench.

Je **ne priorise pas** le TUI comme produit autonome pour l’instant. Quand l’IDE sera plus stable, je pourrai **reprendre** cette surface (multi-pane, polish, signing, etc.). En attendant, elle reste ouverte, utile, et volontairement rude.

Pendant le développement, **Cursor a été un outil**, pas le propriétaire du projet :

- il **traduit mes demandes en code** ; la **maîtrise des features** et les arbitrages restent les miens ;
- il m’a servi à de **grosses phases de réflexion** ;
- il me permet de **tester rapidement des architectures** qui m’auraient pris des mois à la main.

C’est précisément ce confort — accélérer la pensée et l’expérimentation — que je vise à rendre **local et ouvert** avec Drox, pour moi et pour d’autres.

---

## Présentation technique (cours express)

Public : débutant motivé ou développeur confirmé. Le détail opératoire vit dans [`docs/engine/`](docs/engine/README.md).

### 1. Trois briques, un poste de travail

```text
Toi + ton repo
      ↕
 drox-tui  (Ratatui — UI terminal)
      ↕  même process (pas de JSON-RPC)
 drox-engine  (boucle Agent, tools, permissions)
      ↕  HTTP localhost (ou ton endpoint)
 Ollama / LLM que tu as choisi
```

- **TUI** : fil, composer, slash, overlays (`drox/crates/drox-tui/`).
- **Moteur** : décide les tours LLM, appelle les outils, gère permissions / contexte / session.
- **Modèle** : hors du dépôt — tu branches le serveur d’inférence.

Contrairement à l’IDE (process `drox --serve` + Electron), le TUI **embarque** le moteur dans le même binaire.

### 2. Pourquoi Rust ?

- **Un binaire** (`drox-tui` / `.exe`) portable.
- **Perf et mémoire** prévisibles sur des boucles longues (stream, tools, compaction).
- **Contrôle** : erreurs typées, async Tokio, même workspace que l’IDE — je ne duplique pas le cerveau.

### 3. Pourquoi cette arborescence de crates ?

Workspace : `drox/`.

```text
drox-types          contrats partagés
drox-llm            clients Ollama / OpenAI-compat
drox-tools (+ bash, mcp, permissions, hooks, context, session)
drox-engine         boucle Agent (drive_inner), events
drox-cli            binaire + serveur JSON-RPC (pour l’IDE)
drox-tui            client terminal
```

Dépendances **unidirectionnelles** : une crate feuille ne tire pas le monolithe.

### 4. Qu’est-ce qu’un « run » ?

1. Tu envoies un message dans le composer.
2. `EngineRuntime::spawn_run` → `drive_inner` : contexte → stream LLM → tools locaux → nudges.
3. Des `AgentEvent` alimentent le fil ; le run se termine sur un stop / erreur / annulation.

Pipeline actuel : **`tui_mono`**.

### 5. Où lire la suite

| Sujet | Doc |
|-------|-----|
| Crates & clients | [architecture-overview.md](docs/engine/architecture-overview.md) |
| Boucle agent | [agent-run-loop.md](docs/engine/agent-run-loop.md) |
| Outils & permissions | [tools-and-permissions.md](docs/engine/tools-and-permissions.md) |
| Sessions | [sessions-and-memory.md](docs/engine/sessions-and-memory.md) |
| LLM | [llm-backends.md](docs/engine/llm-backends.md) |
| Wiring TUI | [tui-integration.md](docs/engine/tui-integration.md) |
| JSON-RPC (IDE) | [jsonrpc-protocol.md](docs/engine/jsonrpc-protocol.md) |
| Glossaire | [glossary.md](docs/engine/glossary.md) |
| Build / release | [packaging/RELEASE.md](packaging/RELEASE.md) |

Hub doc : [`docs/`](docs/README.md).

### Installer (binaire)

1. Terminal moderne (Windows Terminal recommandé).
2. Un LLM : Ollama local (`ollama pull qwen2.5-coder:7b` pour tester).
3. [Dernière release](https://github.com/DroxKiwi/Drox---TUI/releases/latest) — `.exe` Windows ou `.tar.gz` Linux (`./install.sh`).

Puis : `drox-tui` → `/server` ou `Ctrl+Shift+L` → ouvre ton workspace.  
Installateur Windows **non signé** (SmartScreen possible).

---

## Remerciements

Je remercie l’équipe du **SI de [Salesky](https://www.salesky.fr/)** pour l’aide aux tests et pour le matériel mis à disposition — indispensable pour avancer sur cette quête d’autonomie face à des outils comme Cursor.

---

## Licence

MIT. Portions moteur / TUI / branding © KDDS.  
Détail : [`NOTICE.md`](NOTICE.md) · [`LICENSE`](LICENSE).
