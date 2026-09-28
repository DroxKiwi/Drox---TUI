# Drox TUI

Client **terminal** pour l’agent Drox — open source, 100 % local, même moteur Rust que [Drox IDE](https://github.com/DroxKiwi/Drox---IDE).

## But du projet

Je veux un assistant agent **dans le terminal**, entièrement open source, pour travailler sur de vrais projets sans dépendre d’un produit fermé ni d’un cloud imposé.

Principes non négociables :

- **Aucune télémétrie** Drox / KDDS.
- **Aucun appel imposé** vers des clouds tiers pour faire tourner l’agent.
- Sessions, transcripts et réglages **sur ta machine** (`~/.drox/`, `<workspace>/.drox/`).
- Le LLM, c’est **le tien** : [Ollama](https://ollama.com/) en local, ou un endpoint OpenAI-compatible que **tu** configures (`/server`, `Ctrl+Shift+L`).

Exception réseau **opt-in** uniquement : la vérif de version (`/update on`) lit `releases/latest.json` sur **ce** dépôt. Rien d’autre n’est requis pour coder avec l’agent.

| | |
|---|---|
| **Licence** | MIT — [`LICENSE`](LICENSE) · [`NOTICE.md`](NOTICE.md) |
| **Version** | **2.0.5** · moteur agent **1.5.0** (`tui_mono`) |
| **Binaires** | [Releases](https://github.com/DroxKiwi/Drox---TUI/releases) |
| **Sœur** | [Drox IDE](https://github.com/DroxKiwi/Drox---IDE) (fork Code OSS, même cerveau) |
| **État** | Expérimental / dogfood — bugs et cassures possibles |

```bash
# Sources (Rust ≥ 1.85)
cd drox && cargo build -p drox-tui --release
./target/release/drox-tui
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
| **21 juin** | **2.0.3** : `/update` opt-in, check semver, bandeau MAJ, install SHA256, adaptation Linux. |
| **21 juin** | **2.0.4** : **diff visuel** (`/diff`, overlay, navigation, thème, file_write). |
| **21 juin – 1er juil.** | **2.0.5** : **connexions LLM** self-hosted et cloud (Ollama, Mistral, OVH, HF, Scaleway…), pipeline publish Win/Linux, guide débutant. |

### Septembre 2026 — ouverture totale

| Date | Étape |
|------|--------|
| **28 septembre** | Décision produit : **dévoiler complètement** le code. Canal Releases sur **ce** dépôt (plus le miroir OR). Préparation publique alignée sur Drox IDE. |

Aujourd’hui je pense ce dépôt pour la communauté : cloner, forker, lire, contribuer — ou simplement télécharger l’installeur.

---

## Pourquoi cet outil existe

Des outils comme **Cursor** ont montré qu’un agent change le quotidien. Ils restent des produits d’entreprise : compte, cloud, règles du vendeur.

Drox TUI est né de mon besoin de **retrouver un confort comparable** — certes avec moins de puissance brute — **sans dépendre d’une société** pour le cœur local. Le terminal est l’endroit le plus simple pour dogfooder le moteur avant (et à côté de) l’IDE.

Pendant le développement, **Cursor a été un outil**, pas le propriétaire du projet :

- il **traduit mes demandes en code** ; la **maîtrise des features** et les arbitrages restent les miens ;
- il m’a servi à de **grosses phases de réflexion** ;
- il me permet de **tester rapidement des architectures** qui m’auraient pris des mois à la main.

C’est ce confort que je vise à rendre **local et ouvert** avec Drox, pour moi et pour d’autres.

---

## Présentation technique (cours express)

### 1. Trois briques

```text
Toi + ton repo
      ↕
 drox-tui  (UI terminal Ratatui)
      ↕  même process / crates
 drox-engine (boucle Agent, tools, permissions)
      ↕  HTTP localhost (ou ton endpoint)
 Ollama / LLM que tu as choisi
```

Contrairement à l’IDE (process `drox --serve` + Electron), le TUI **embarque** le moteur dans le même binaire.

### 2. Pourquoi Rust ?

- Un **seul binaire** portable (`drox-tui` / `.exe`).
- Perf et mémoire prévisibles sur des boucles longues.
- Même workspace crates que le moteur IDE : `drox-types` → `drox-llm` → `drox-tools` → `drox-engine` → `drox-tui` / `drox-cli`.

### 3. Workspace (`drox/`)

```text
drox-types          contrats partagés
drox-llm            clients Ollama / OpenAI-compat
drox-tools (+ bash, mcp, permissions, hooks, context, session)
drox-engine         boucle Agent (drive_inner), events
drox-cli            binaire serveur JSON-RPC (pour l’IDE)
drox-tui            client terminal
```

Pipeline courant : **`tui_mono`** — une boucle claire.

### 4. Packaging

| Script | Rôle |
|--------|------|
| `packaging/build-and-pack.ps1` | Build + installateur Windows (Inno Setup) |
| `packaging/build-and-pack-linux.sh` | Build + `tar.gz` Linux |
| `packaging/RELEASE.md` | Guide opérateur |

---

## Installer (débutant)

1. Terminal moderne (Windows Terminal recommandé).
2. Un LLM : Ollama local recommandé (`ollama pull qwen2.5-coder:7b` pour tester).
3. Télécharge la [dernière release](https://github.com/DroxKiwi/Drox---TUI/releases/latest) :
   - **Windows** : `drox-tui-*-windows-x64-setup.exe`
   - **Linux** : `drox-tui-*-linux-x64.tar.gz` puis `./install.sh`

Ensuite : lance `drox-tui`, configure le serveur LLM (`Ctrl+Shift+L` ou `/server`), ouvre ton workspace.

Installateur Windows **non signé** (SmartScreen « éditeur inconnu » possible) — signing prévu plus tard.

---

## Remerciements

Je remercie l’équipe du **SI de [Salesky](https://www.salesky.fr/)** pour l’aide aux tests et pour le matériel mis à disposition — indispensable pour avancer sur cette quête d’autonomie face à des outils comme Cursor.

---

## Licence

MIT. Portions moteur / TUI / branding © KDDS.  
Détail : [`NOTICE.md`](NOTICE.md) · [`LICENSE`](LICENSE).
