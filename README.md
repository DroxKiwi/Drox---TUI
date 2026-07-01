# Drox TUI — agent local en terminal

> ⚠️ **Avertissement** — Drox TUI est **potentiellement instable** : développement actif et exigeant, produit encore **expérimental**. Prochaines mises à jour : **2.0.6** multi-pane, **2.0.7** signature installateur et GPG Linux, etc. Il est conseillé d’utiliser les [releases](https://github.com/DroxKiwi/Drox---TUI---OR/releases) et de s’attendre à des évolutions fréquentes.
>
> ⚠️ **Warning** — Drox TUI may be **unstable**: active, demanding development; still **experimental**. Upcoming: **2.0.6** multi-pane, **2.0.7** installer signing and Linux GPG, etc. Use [releases](https://github.com/DroxKiwi/Drox---TUI---OR/releases) and expect frequent changes.

> **Tu découvres Drox ?** **Drox TUI** est un assistant agent en **terminal**, adapté aux débutants.  
> **Produit sœur** : **[Drox IDE](https://github.com/DroxKiwi/Drox---IDE---OR)** (éditeur graphique, fork VS Code, même moteur agent) — **beaucoup plus difficile à prendre en main** (install lourde, UI riche, toujours expérimental). **Il est conseillé de commencer par le TUI**.

> Dépôt de **développement** — binaires et notes de release sur [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR).

## But du projet — souveraineté et feuille de route

### Où en est Drox TUI (2.0.x)

Le TUI est le **cœur agent d’origine** de l’écosystème Drox : une boucle mono-agent (`tui_mono`) en Rust, dérivée du moteur **Drox IDE 1.5.0**, emballée en binaire terminal autonome. La release courante [**2.0.5**](https://github.com/DroxKiwi/Drox---TUI---OR/releases/latest) apporte les **connexions LLM** self-hosted et cloud (Ollama, Mistral, OVH, HF, Scaleway…) ; les lignes **2.0.6** (poste multi-pane) et **2.0.7** (signature installateur, GPG Linux) suivent. Toujours **expérimental** — assistant terminal pour early adopters, pas un produit agent « prod ».

### Souveraineté

Drox TUI vise la **souveraineté numérique** : agent et sessions **en local**, LLM via **Ollama** (ou API que tu configures), données dans **`~/.drox/`** et **`<workspace>/.drox/`** sur ton disque — pas de compte cloud KDDS obligatoire, pas de télémétrie Drox, pas de « phone home ».

**Seule exception réseau côté produit** : la **vérification de version** (lecture de `releases/latest.json` sur le dépôt OR) **si tu l’actives** (`/update on`). Rien d’autre n’est requis pour coder avec l’agent. L’agent n’utilise le réseau que pour ton LLM distant ou les outils web/MCP que **tu** autorises.

### Vision produit

Permettre de **maîtriser des projets volumineux depuis le terminal** avec une **IA légère** (modèles locaux ou petits modèles distants) — peu gourmande en RAM/VRAM et en tokens. Comprendre d’abord (lecture, grep, plan), agir ensuite (fichiers, bash), avec **permissions explicites** et **observabilité locale** (fil, phases, diffs) pour rester maître du système.

Le TUI sert aussi de **référence UX** pour Drox IDE : le fil chronologique, les phases et les outils agent sont alignés sur cette base.

### Pistes à venir (brainstorm)

Plans produit dans ce dépôt — voir aussi la vision partagée [Drox IDE](https://github.com/DroxKiwi/Drox---IDE) ([index brainstorm](https://github.com/DroxKiwi/Drox---IDE/blob/main/drox-engine/docs/feature-brainstorm/README.md)) :

| Thème | Objectif | Ligne |
|-------|----------|-------|
| **Diff visuel overlay** | `/diff`, viewer coloré, `e` sur les patches | ✅ **2.0.4** |
| **Connexions LLM** | Self-hosted + cloud (Ollama, Mistral, OVH, HF, Scaleway…) | ✅ **2.0.5** |
| **Poste multi-pane** | Fil · carte contexte · changements+diff | **2.0.6** |
| **Confiance install** | Authenticode Windows, signatures GPG Linux | **2.0.7** |
| **Télémétrie locale** | KPI par run, dashboards **100 % locaux** (`.drox/`) — aucun cloud | piste |
| **i18n FR/EN** | Interface et messages utilisateur bilingues | en cours |
| **Animation démarrage** | Splash ASCII / halo au boot | `docs/animation-start/` |
| **Sessions & long run** | Gros chantiers multi-heures ; reprise historique | en cours |
| **IA légère & perf** | Réponses rapides ; presets modèle par rôle | aligné IDE |

Ces pistes **ne bloquent pas** les releases courantes ; elles nourrissent la ligne **2.0.x+**.

---

## ⚠️ STATUT — version 2.0.5 (juin 2026)

> **Drox TUI 2.0.5** : connexions LLM self-hosted & cloud, fix auth Bearer Ollama Cloud, wizard `/server` par prestataire — moteur **1.5.0** (`tui_mono`).  
> Toujours **expérimental** — early adopters / dogfood.

| | |
|---|---|
| **Version** | [**2.0.5**](https://github.com/DroxKiwi/Drox---TUI---OR/releases/latest) (juin 2026) · moteur **1.5.0** |
| **Plateformes** | **Windows** installateur 2.0.5 · **Linux** tar.gz selon release OR |
| **Utilisable au quotidien ?** | **Partiellement** — fonctionnel pour le travail agent local ; polish et signing en cours. |
| **Nouveautés 2.0.5** | `/server` cloud + self-hosted · Bearer Ollama Cloud · Mistral/OVH/HF/Scaleway · headers custom. |
| **2.0.x−** | **2.0.4** : diff overlay. **2.0.3** : `/update` opt-in. **2.0.2** : i18n. |

**En bref** : *le terminal où vit la boucle agent Drox — connexions LLM locales et cloud enfin utilisables.*

---

**Releases officielles** : [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR) · notes [2.0.5](https://github.com/DroxKiwi/Drox---TUI---OR/blob/main/releases/v2.0.5/RELEASE_NOTES.md)

| | |
|---|---|
| Installer | [Télécharger](https://github.com/DroxKiwi/Drox---TUI---OR/releases/latest) |
| MAJ auto | `releases/latest.json` (opt-in `/update on`) |
| Ollama (recommandé) | [ollama.com](https://ollama.com/) |
| SmartScreen | Installeur **non signé** — « Éditeur inconnu » normal · signing prévu **2.0.7** |

---

## Guide débutant — installer et démarrer

Tu n’as jamais utilisé Drox TUI ? Suis ces étapes dans l’ordre.

### Ce qu’il te faut

1. Un **terminal moderne** (Windows Terminal est recommandé sur Windows).
2. Un **moteur d’inférence** configuré — voir [Moteurs d’inférence](#moteurs-dinférence-ollama-vllm) ci-dessous (Ollama est recommandé pour débuter).

### Moteurs d’inférence (Ollama, vLLM)

Drox TUI **n’embarque pas** de modèle : il appelle un **serveur LLM** que tu choisis via **`Ctrl+Shift+L`** ou **`/server`**. Deux familles courantes :

| Moteur | En local (chez toi) | En cloud (distant) |
|---|---|---|
| **[Ollama](https://ollama.com/)** | `http://127.0.0.1:11434` — modèles sur ton PC / serveur ([matériel testé](#matériel-testé-inférence-locale)) | **[Ollama Cloud](https://ollama.com/cloud)** — modèles hébergés, connexion par clé API (preset *Ollama Cloud*) |
| **[vLLM](https://docs.vllm.ai/)** | API **OpenAI-compatible** sur ta machine (ex. `http://127.0.0.1:8000/v1`) — un GPU est recommandé ([matériel testé](#matériel-testé-inférence-locale)) | Instance vLLM sur **ton** VPS, cluster ou cloud privé (même API, URL + token) |

Presets intégrés dans la modale connexion : *Ollama local*, *Ollama Cloud*, *Mistral*, *OVHcloud AI Endpoints*, *Hugging Face*, *Scaleway*, *vLLM (OpenAI)*, *LM Studio*, *OpenAI-compatible*.

**Local vs cloud — ce que ça implique**

| | Local | Cloud (Ollama Cloud, vLLM hébergé, …) |
|---|---|---|
| **Données** | Prompts et réponses restent sur **ton** matériel | Requêtes envoyées au **fournisseur** que tu configures |
| **Confidentialité** | Souveraineté maximale (alignée Drox) | Dépend du **contrat du service** : Ollama et les hébergeurs vLLM annoncent transport chiffré (HTTPS) et politiques de non-rétention **selon leurs propres docs** — à lire avant usage pro |
| **Choix** | **100 % opt-in** : Drox n’impose aucun cloud KDDS ; tu saisis l’URL et la clé |

Pour commencer en local avec Ollama (voir [matériel testé](#matériel-testé-inférence-locale)) :

```bash
# Principal (24 Go VRAM+, ex. RTX 3090) — qwen3.6:27b, Q4_K_M, de préférence variante MTP
ollama pull qwen3.6:27b

# Alternative — gemma4:26b, Q4_K_M, format it-qat
ollama pull gemma4:26b

# Laptop / essais — petits modèles orientés spé code à essayer aussi :
ollama pull qwen2.5-coder:7b
# ou : ollama pull gemma3:4b
```

Puis dans Drox TUI : adresse `http://127.0.0.1:11434`, modèle = tag Ollama choisi.

### Modèles testés pendant le dev

Le développement de Drox TUI s’appuie surtout sur :

| Profil | Modèle | Quantification | Format / remarque |
|---|---|---|---|
| **Station** | **`qwen3.6:27b`** | **Q4_K_M** | de préférence variante **MTP** |
| **Alternative** | **`gemma4:26b`** | **Q4_K_M** | format **it-qat** |
| **Portable / essais** | modèles plus petits orientés spé code | Q4/Q5 selon VRAM | ex. `qwen2.5-coder:7b`, `gemma3:4b` — également adaptés |

### Matériel testé (inférence locale)

Drox TUI tourne sur **toute machine** où Ollama ou vLLM fonctionne. Les configurations suivantes ont été **testées** (Windows, Ollama local, agent sur de vrais dépôts) :

| Profil | Machine | GPU | VRAM | RAM système |
|---|---|---|---|---|
| **Station** | PC bureau | NVIDIA **RTX 3090** | **24 Go** | **96 Go** |
| **Portable** | **MSI Helios AI 16** | NVIDIA **RTX 5070 Ti** | **12 Go** | *(selon config)* |

**Synthèse**

- **RTX 3090 + 96 Go RAM** — config de référence pour le dev : **`qwen3.6:27b`** en **Q4_K_M** (de préférence **MTP**), sinon **`gemma4:26b`** en **Q4_K_M** (**it-qat**). Contextes longs et runs agent intensifs sans stress.
- **RTX 5070 Ti 12 Go** (Helios AI 16) — laptop récent : **tout à fait utilisable** avec des modèles **plus petits orientés spé code** (`qwen2.5-coder:7b`, `gemma3:4b`, …) ; ces modèles conviennent aussi sur station pour des runs plus rapides ; contexte modéré dans **`/server`**.

**Repères généraux (local, indicatif)**

| VRAM GPU | Modèles Ollama raisonnables |
|---|---|
| **12 Go** | 4B–7B code (Q4/Q5), 14B légers avec contexte réduit |
| **24 Go** | **27B–26B en Q4_K_M** (config de référence dev), ou 7B–14B confort |

La **RAM système** compte aussi (contexte, outils, IDE terminal) — il est recommandé d’avoir **32 Go+** en local ; **96 Go** apportent de la marge sur gros workspaces.

Ce ne sont **pas** des minimums officiels Drox : lance **`/doctor`**, teste ton modèle, ajuste `num_ctx` dans la connexion IA. Une machine plus modeste peut suffire avec un petit modèle ; une config cloud reste toujours possible.

### Étape 1 — Installer Drox TUI

#### Windows x64

1. Télécharge [`drox-tui-2.0.5-windows-x64-setup.exe`](https://github.com/DroxKiwi/Drox---TUI---OR/releases/tag/v2.0.5) depuis [Releases](https://github.com/DroxKiwi/Drox---TUI---OR/releases/latest).
2. Lance l’installateur → coche **Ajouter au PATH** si proposé.
3. Ouvre un **nouveau** terminal (obligatoire pour que `drox-tui` soit reconnu).

Windows peut afficher « Éditeur inconnu » (SmartScreen) — l’installeur n’est pas encore signé ; tu peux continuer.

#### Linux x64

```bash
# Archive Linux : voir [Releases](https://github.com/DroxKiwi/Drox---TUI---OR/releases) (version 2.0.5 si publiée)
tar xzf drox-tui-2.0.3-linux-x64.tar.gz
cd drox-tui-2.0.3-linux-x64
./install.sh
```

Vérifie : `drox-tui --version`

### Étape 2 — Lancer et connecter l’IA

Place-toi dans **ton projet** (dossier de code) :

```powershell
# Windows
cd C:\Users\toi\mon-projet
drox-tui --workspace .
```

```bash
# Linux / macOS / WSL
cd ~/mon-projet
drox-tui --workspace .
```

Au **premier lancement**, la modale **Connexion IA** s’ouvre (sinon : **`Ctrl+Shift+L`** ou tape `/server`) :

| Champ | Valeur habituelle |
|---|---|
| Adresse serveur | `http://127.0.0.1:11434` |
| Modèle | celui que tu as pull (ex. `qwen3.6:27b` sur station, `qwen2.5-coder:7b` sur laptop) |

Clique **Tester**, puis **Enregistrer**. La configuration reste dans `~/.drox/tui-preferences.json`.

### Étape 3 — Ton premier message

1. Écris une demande simple dans le **composer** (zone de saisie en bas), par ex. *« Résume la structure de ce dépôt »*.
2. Appuie sur **Entrée**.
3. Observe le **fil** au centre : phases repliables, outils (`file_read`, `grep`, …), réponse finale.
4. Si l’agent veut **modifier un fichier** ou **lancer une commande**, une **fenêtre permission** s’ouvre — lis l’aperçu (diff, commande bash) puis **Accepte** ou **Refuse**.

Tu n’es pas obligé d’accepter : c’est le comportement normal en mode prudent.

### Étape 4 — L’écran en un coup d’œil

```text
┌─ En-tête — workspace, modèle, [MAJ on/off] ─────────────┐
│ Fil — conversation, outils agent, diffs (défile)         │
│ Composer — tu écris ici                                  │
└─ Barre du bas — durée, tokens, mode permission ──────────┘
```

### Étape 5 — Indispensables

| Tu veux… | Fais… |
|---|---|
| Voir toutes les commandes | `/help` |
| Vérifier Ollama / environnement | `/doctor` |
| Voir les fichiers modifiés | `/diff` ou touche **`e`** sur un diff dans le fil |
| Changer de modèle ou serveur | **`Ctrl+Shift+L`** |
| Changer de projet | **`Ctrl+Shift+W`** ou `/workspace` |
| Annuler un run en cours | **`Esc`** ou **`Ctrl+C`** |
| Chercher dans la conversation | **`Ctrl+F`** |

Tape **`/`** dans le composer pour la palette de commandes (`/settings`, `/theme`, `/permissions`, …).

### Étape 6 — Modes (ne pas se précipiter)

| Mode | Comment | Effet |
|---|---|---|
| **Prudent** (recommandé par défaut) | `drox-tui --workspace .` | L’agent **demande** avant d’écrire ou d’exécuter bash |
| **Plan** | ajoute `--plan` | Lecture et conseil — pas d’écriture sur disque |
| **Apply** | ajoute `--apply` | Écritures réelles — à n’utiliser que quand tu fais confiance au run |

**Il est conseillé** de rester sans `--apply` les premières sessions ; valide chaque action via les modales.

### Étape 7 — Reprendre une session

```bash
drox-tui --list-sessions
drox-tui --workspace . --session ses_XXXXXXXX
```

Les transcripts sont dans `~/.drox/sessions/`.

### Problème ?

- Ollama ne répond pas → `ollama serve` ou relance l’app Ollama, puis `/doctor`.
- Commande introuvable → nouveau terminal après install Windows, ou `~/.local/bin` dans le PATH (Linux).
- [Issues](https://github.com/DroxKiwi/Drox---TUI---OR/issues) · notes [2.0.5](https://github.com/DroxKiwi/Drox---TUI---OR/blob/main/releases/v2.0.5/RELEASE_NOTES.md)

---

## FR — Vue globale

Tu installes **`drox-tui`**, tu fais tourner **Ollama** avec un modèle (ex. **`qwen3.6:27b`**, **`gemma4:26b`**, ou un petit modèle code), tu ouvres ton projet en terminal. Chaque message lance **une boucle agent locale** : le TUI appelle ton modèle, exécute les outils (fichiers, bash, grep, web avec permission) et affiche le fil en streaming.

**La pile**

```mermaid
flowchart LR
  DEV(["Toi + ton repo"])
  TUI(["drox-tui"])
  OLL(["Ollama"])
  MDL(["Ton modele"])
  DISK([".drox/ sessions"])

  DEV <-->|terminal| TUI
  TUI <-->|HTTP localhost| OLL
  OLL --- MDL
  TUI --> DISK
  TUI -->|file_read bash file_write| DEV
```

**Un message dans le TUI**

```mermaid
flowchart TB
  U(["Tu envoies un message"])
  T(["Drox TUI"])
  M(["Boucle agent TUI"])
  L(["LLM via Ollama"])
  O(["Outils locaux"])

  U --> T
  T --> M
  M -->|prompt + historique| L
  L -->|phase: + texte + tool_calls| M
  M --> O
  O -->|resultats| M
  M --> T
  T --> U
```

| Brique | Rôle |
|--------|------|
| **Ollama** | Inférence locale — le modèle que **tu** choisis |
| **drox-tui** | Boucle agent, UI terminal, permissions, sessions |
| **Toi** | Repo, modèle, modes `--plan` / `--apply`, règles allow/ask/deny |

Pas de compte cloud KDDS obligatoire. Données session dans **`~/.drox/`** et **`<workspace>/.drox/`**.

---

## FR — Lancer Drox TUI

Après installation, la commande est **`drox-tui`** :

```bash
# Linux / macOS / WSL
cd /chemin/vers/votre-projet
drox-tui --workspace .
```

```powershell
# Windows
cd C:\chemin\vers\votre-projet
drox-tui --workspace .
```

Sans `--workspace`, le répertoire courant est utilisé. Au **premier lancement**, configurez Ollama via **`Ctrl+Shift+L`** ou **`/server`**.

---

## FR — Installation

**Prérequis** : [Ollama](https://ollama.com/) (ou serveur compatible), terminal moderne (Windows Terminal est recommandé sur Windows).

### Windows x64

1. Téléchargez `drox-tui-2.0.5-windows-x64-setup.exe` depuis [Releases](https://github.com/DroxKiwi/Drox---TUI---OR/releases/tag/v2.0.5).
2. Lancez l’**installateur** (double-clic) et suivez l’assistant.
3. Cochez **Ajouter au PATH** si proposé.

Le binaire est installé dans `%LOCALAPPDATA%\Programs\DroxTUI\bin`. Ouvrez un **nouveau** terminal.

### Linux x64

1. Téléchargez `drox-tui-2.0.5-linux-x64.tar.gz` depuis [Releases](https://github.com/DroxKiwi/Drox---TUI---OR/releases) (si publiée).
2. Extrayez et installez :

```bash
tar xzf drox-tui-2.0.3-linux-x64.tar.gz
cd drox-tui-2.0.3-linux-x64
./install.sh              # ~/.local/bin
# ou : ./install.sh --system   # /usr/local/bin (sudo)
drox-tui --workspace ~/projets/mon-repo
```

Vérifiez : `drox-tui --list-sessions`

---

## FR — Démarrage rapide

| Option | Rôle |
|---|---|
| `--workspace` | Dossier de travail de l’agent |
| `--apply` | Autorise les écritures fichier réelles |
| `--plan` | Mode plan (pas d’écriture sans validation) |
| `--session ses_…` | Reprend une session |
| `--list-sessions` | Liste les sessions |
| `-v` | Logs dans `~/.drox/tui.log` |

Variables d’environnement : `DROX_SERVER`, `DROX_MODEL`, `DROX_WORKSPACE`, `DROX_API_KEY`.

Au premier lancement, la modale **Connexion IA** enregistre la config dans `~/.drox/tui-preferences.json`.

---

## FR — Architecture : une boucle, un binaire

Contrairement à Drox IDE (Electron + `drox.exe` + shim RPC), le TUI **embarque directement** la boucle agent **1.5.0** — pipeline **`tui_mono`**, sans couche IDE intermédiaire.

```mermaid
flowchart TB
  subgraph TUI["drox-tui — terminal"]
    COMP["Composer + slash commands"]
    FIL["Fil streaming + phases"]
    PERM["Modales permission + diff"]
  end

  subgraph CORE["Coeur agent — agent.rs"]
    LOOP["Boucle LLM + outils + permissions"]
    PH["Phases phase: …"]
  end

  subgraph EXT["Externe"]
    OLL["Ollama / API compatible"]
    FS["Workspace + .drox/"]
  end

  COMP --> LOOP
  FIL --> LOOP
  PERM --> LOOP
  LOOP --> PH
  LOOP <-->|stream| OLL
  LOOP --> FS
```

| Couche | Responsabilité |
|--------|----------------|
| **UI terminal** | Composer, fil, overlays (`/diff`), modales, thèmes, raccourcis |
| **Cœur agent** | Tours LLM, protocole `[phase: …]`, outils, permissions, session, compaction |
| **Ollama** | Inférence — prompts + schémas outils ; tokens + `tool_calls` |

Chaque envoi déclenche **un run** jusqu’à **`[phase: done]`**. Modes permission : **`--plan`** (lecture/conseil), **`--apply`** (écritures auto), défaut (allow/ask/deny).

---

## FR — Phases agent

Le modèle structure son travail avec des lignes **`[phase: nom]`**. Seul **`[phase: done]`** clôt le run. Le texte sous **`[phase: answering]`** est la réponse visible.

```mermaid
flowchart LR
  A[analyzing] --> R[reading]
  R --> C[clarifying]
  C --> P[planning]
  P --> X[acting]
  R --> X
  X --> T[testing]
  T --> V[verifying]
  X --> V
  V --> AN[answering]
  AN --> D[done]
```

Les phases intermédiaires sont **optionnelles** ; le chemin dépend de la tâche.

---

## FR — Fonctionnalités

- **Fil de conversation** : streaming, markdown, outils, phases repliables
- **Diff visuel** : `/diff`, `/diff --stat`, `/diff <fichier>`, touche **`e`**, bandeau fin de run
- **Composer** : vim (`/vim`), `@fichier`, collage intelligent, images (Ollama)
- **Agent** : lecture/écriture fichiers, grep, bash, web (avec permission), MCP, todos, plan
- **Permissions** : modales avec aperçu (diff, commande bash, plan)
- **Sessions** : reprise, export, rewind
- **Personnalisation** : thèmes, raccourcis clavier, workspaces récents, i18n FR/EN

Sans `--apply`, les modifications fichier sont **proposées** mais non appliquées sur disque.

---

## FR — Raccourcis et commandes

| Raccourci | Action |
|---|---|
| `Ctrl+Shift+L` | Connexion serveur IA |
| `Ctrl+Shift+W` | Changer de workspace |
| `Ctrl+F` | Rechercher dans le fil |
| `Ctrl+R` | Recherche dans l’historique |
| `Esc` / `Ctrl+C` | Annuler un run |
| `e` | Agrandir diff / sortie d’outil |
| `/help` | Liste des commandes slash |

Commandes utiles : `/server`, `/workspace`, `/settings`, `/theme`, `/permissions`, `/diff`, `/compact`, `/export`, `/doctor`, `/update`.

---

## FR — Garde-fous

| Mécanisme | Effet |
|-----------|--------|
| Modes `--plan` / `--apply` / défaut | Contrôle des écritures et bash |
| Permissions chemins + bash | Allow / ask / deny sur le workspace |
| `[phase: done]` | Clôture explicite du run |
| Compaction contexte | Snip / résumé quand la fenêtre LLM déborde |
| Modales permission | Aperçu diff/bash avant acceptation |

---

## FR — Persistance session

```text
~/.drox/
├── tui-preferences.json   # LLM, thème, MAJ
├── keybindings.json
├── settings.json
├── sessions/              # transcripts JSONL
└── tui.log                # logs (-v)

<workspace>/.drox/         # réglages et mémoire projet
```

---

## FR — Mises à jour

Par défaut, **aucune** vérification automatique.

- **`/update on`** / **`/update off`** — opt-in ; pastille header `[MAJ on]` / `[MAJ off]`
- **`/update check`** — contrôle manuel vers `releases/latest.json`
- **`/update install`** — téléchargement HTTPS, SHA256, confirmation
- **`Ctrl+Shift+U`** — installer ou snooze

Seule communication produit Drox vers l’extérieur : **vérification de version** sur le dépôt OR (opt-in).

---

## FR — Ce que le produit n’est pas

| Pas | Détail |
|-----|--------|
| Agent terminal « prod » | Toujours expérimental — mais base 2.0.x solide |
| IDE graphique | Voir [Drox IDE](https://github.com/DroxKiwi/Drox---IDE---OR) pour l’éditeur |
| Cloud Drox obligatoire | LLM et données restent chez toi |
| Installateur signé | Prévu **2.0.7** (Authenticode + GPG) |
| Code source moteur ouvert | — |

---

---

> **New to Drox?** **Drox TUI** is a **terminal** agent, well suited for beginners.  
> **Sibling product**: **[Drox IDE](https://github.com/DroxKiwi/Drox---IDE---OR)** (graphical editor, VS Code fork, same agent engine) — **much harder to get started with** (heavy install, rich UI, still experimental). **Starting with the TUI is recommended**.

## EN — Project goal — sovereignty and roadmap

### Where Drox TUI stands (2.0.x)

The TUI is the **original agent core** of the Drox ecosystem: a Rust **mono-agent loop** (`tui_mono`), derived from **Drox IDE 1.5.0**, shipped as a standalone terminal binary. Current release [**2.0.5**](https://github.com/DroxKiwi/Drox---TUI---OR/releases/latest) adds **LLM connections** (self-hosted and cloud); **2.0.6** (multi-pane) and **2.0.7** (installer signing, Linux GPG) follow. Still **experimental** — not a production daily driver.

### Sovereignty

Drox TUI aims for **digital sovereignty**: local agent and sessions, LLM via **Ollama** (or an API you configure), data in **`~/.drox/`** and **`<workspace>/.drox/`** — no mandatory KDDS cloud account, no Drox telemetry, no phone home.

**Only product-side network exception**: **version check** (reading `releases/latest.json` on the OR repo) **if you enable it** (`/update on`). Nothing else is required. The agent uses the network only for your remote LLM or web/MCP tools **you** allow.

### Product vision

**Master large codebases from the terminal** with **lightweight AI** (local or small remote models) — low RAM/VRAM and token use. Understand first (read, grep, plan), act second (files, bash), with **explicit permissions** and **local observability** (stream, phases, diffs).

The TUI is also the **UX reference** for Drox IDE: chronological stream, phases, and agent tools align on this foundation.

### Upcoming themes (brainstorm)

Product plans in this repo — shared vision with [Drox IDE](https://github.com/DroxKiwi/Drox---IDE) ([brainstorm index](https://github.com/DroxKiwi/Drox---IDE/blob/main/drox-engine/docs/feature-brainstorm/README.md)):

| Theme | Goal | Line |
|-------|------|------|
| **Visual diff overlay** | `/diff`, colored viewer, `e` on patches | ✅ **2.0.4** |
| **LLM connections** | Self-hosted + cloud providers | ✅ **2.0.5** |
| **Multi-pane cockpit** | Feed · context map · changes+diff | **2.0.6** |
| **Install trust** | Windows Authenticode, Linux GPG | **2.0.7** |
| **Local telemetry** | Per-run KPIs, **100 % local** dashboards | planned |
| **FR/EN i18n** | Bilingual UI | ongoing |
| **Boot animation** | ASCII splash / halo | `docs/animation-start/` |

These themes **do not block** current releases; they feed **2.0.x+**.

---

## EN — Status (2.0.5)

> **Drox TUI 2.0.5** : self-hosted & cloud LLM connections, Ollama Cloud Bearer fix, `/server` wizard per provider — **1.5.0** engine (`tui_mono`).  
> Still **experimental**.

| | |
|---|---|
| **Version** | [**2.0.5**](https://github.com/DroxKiwi/Drox---TUI---OR/releases/latest) (June 2026) · engine **1.5.0** |
| **Platforms** | **Windows** installer 2.0.5 · **Linux** tar.gz per OR release |
| **Daily driver?** | **Partially** — solid for local agent work; polish and signing in progress. |
| **2.0.5 highlights** | `/server` cloud + self-hosted · Ollama Cloud Bearer · Mistral/OVH/HF/Scaleway · custom headers. |

**In short**: *the terminal home of the Drox agent loop — local and cloud LLM connections that work.*

---

## EN — Getting started (beginner guide)

Never used Drox TUI? Follow these steps in order.

### What you need

1. A **modern terminal** (Windows Terminal is recommended on Windows).
2. An **inference engine** configured — see [Inference engines](#en--inference-engines-ollama-vllm) below (Ollama is recommended to start).

### EN — Inference engines (Ollama, vLLM)

Drox TUI does **not** ship a model: it calls an **LLM server** you choose via **`Ctrl+Shift+L`** or **`/server`**.

| Engine | Local (on your hardware) | Cloud (remote) |
|---|---|---|
| **[Ollama](https://ollama.com/)** | `http://127.0.0.1:11434` ([tested hardware](#en--tested-hardware-local-inference)) | **[Ollama Cloud](https://ollama.com/cloud)** — API key, hosted models |
| **[vLLM](https://docs.vllm.ai/)** | OpenAI-compatible API (e.g. `http://127.0.0.1:8000/v1`) — a GPU is recommended ([tested hardware](#en--tested-hardware-local-inference)) | Your VPS / private cloud instance (URL + token) |

Built-in presets: *Ollama local*, *Ollama Cloud*, *Mistral*, *OVHcloud AI Endpoints*, *Hugging Face*, *Scaleway*, *vLLM (OpenAI)*, *LM Studio*, *OpenAI-compatible*.

**Local vs cloud**

| | Local | Cloud |
|---|---|---|
| **Data** | Prompts stay on **your** machine | Requests go to **your chosen provider** |
| **Privacy** | Maximum sovereignty | Depends on **provider terms** — Ollama and vLLM hosts advertise HTTPS and no-retention policies **per their documentation**; review before professional use |
| **Choice** | **Opt-in only** — no mandatory KDDS cloud |

Quick local start ([tested hardware](#en--tested-hardware-local-inference)):

```bash
# Primary (24 GB VRAM+, e.g. RTX 3090) — qwen3.6:27b, Q4_K_M, MTP variant preferred
ollama pull qwen3.6:27b

# Alternative — gemma4:26b, Q4_K_M, it-qat format
ollama pull gemma4:26b

# Laptop / experiments — smaller code-oriented models also worth trying:
ollama pull qwen2.5-coder:7b
# or: ollama pull gemma3:4b
```

### EN — Models tested during development

Drox TUI development primarily relies on:

| Profile | Model | Quantization | Format / notes |
|---|---|---|---|
| **Desktop** | **`qwen3.6:27b`** | **Q4_K_M** | **MTP** variant preferred |
| **Alternative** | **`gemma4:26b`** | **Q4_K_M** | **it-qat** format |
| **Laptop / tryouts** | smaller code-oriented models | Q4/Q5 by VRAM | e.g. `qwen2.5-coder:7b`, `gemma3:4b` — also suitable |

### EN — Tested hardware (local inference)

Drox TUI runs on **any machine** where Ollama or vLLM works. The following setups have been **validated** (Windows, local Ollama, real repos):

| Profile | Machine | GPU | VRAM | System RAM |
|---|---|---|---|---|
| **Desktop** | Workstation | NVIDIA **RTX 3090** | **24 GB** | **96 GB** |
| **Laptop** | **MSI Helios AI 16** | NVIDIA **RTX 5070 Ti** | **12 GB** | *(varies by SKU)* |

**Summary**

- **RTX 3090 + 96 GB RAM** — reference dev setup: **`qwen3.6:27b`** in **Q4_K_M** (**MTP** preferred), or **`gemma4:26b`** in **Q4_K_M** (**it-qat**). Long contexts and heavy agent runs without stress.
- **RTX 5070 Ti 12 GB** (Helios AI 16) — **fully usable** with **smaller code-oriented models** (`qwen2.5-coder:7b`, `gemma3:4b`, …); these models also work on desktop setups for faster runs; moderate context in **`/server`**.

| GPU VRAM | Reasonable Ollama models (indicative) |
|---|---|
| **12 GB** | 4B–7B code (Q4/Q5), light 14B with reduced context |
| **24 GB** | **27B–26B in Q4_K_M** (reference dev setup), or comfortable 7B–14B |

System **RAM** matters too (context, tools, terminal IDE) — **32 GB+** is recommended locally; **96 GB** allows headroom on large workspaces.

These are **not** official Drox minimums — run **`/doctor`**, test your model, tune `num_ctx`. Smaller hardware can work with tiny models; cloud inference remains an option.

### Step 1 — Install Drox TUI

**Windows**: download [`drox-tui-2.0.5-windows-x64-setup.exe`](https://github.com/DroxKiwi/Drox---TUI---OR/releases/tag/v2.0.5), run the installer, enable **Add to PATH**, open a **new** terminal.

**Linux**: extract `drox-tui-2.0.3-linux-x64.tar.gz`, run `./install.sh`, check `drox-tui --version`.

SmartScreen may warn « Unknown publisher » — expected until signing lands in 2.0.7.

### Step 2 — Launch and connect AI

```bash
cd /path/to/your-project
drox-tui --workspace .
```

On first launch, the **AI connection** dialog opens (or **`Ctrl+Shift+L`** / `/server`):

| Field | Typical value |
|---|---|
| Server | `http://127.0.0.1:11434` |
| Model | your Ollama tag (e.g. `qwen3.6:27b` on desktop, `qwen2.5-coder:7b` on laptop) |

Click **Test**, then **Save**.

### Step 3 — First message

1. Type a simple request in the **composer** (bottom), e.g. *“Summarize this repo structure”*.
2. Press **Enter**.
3. Watch the **stream**: phases, tools (`file_read`, `grep`, …), final answer.
4. When the agent wants to **write a file** or **run bash**, a **permission modal** appears — preview, then accept or deny.

### Step 4 — Screen layout

```text
┌─ Header — workspace, model, [MAJ on/off] ───────────────┐
│ Stream — conversation, tools, diffs (scroll)             │
│ Composer — you type here                                 │
└─ Status bar — duration, tokens, permission mode ─────────┘
```

### Step 5 — Essentials

| Goal | Action |
|---|---|
| All slash commands | `/help` |
| Check Ollama / env | `/doctor` |
| See file changes | `/diff` or **`e`** on a diff block |
| Change model / server | **`Ctrl+Shift+L`** |
| Cancel run | **`Esc`** |

### Step 6 — Modes

| Mode | Command | Effect |
|---|---|---|
| **Safe** (recommended default) | `drox-tui --workspace .` | Prompts before writes / bash |
| **Plan** | add `--plan` | Read-only advice |
| **Apply** | add `--apply` | Real file writes — use when you trust the run |

**It is recommended** to stay **without `--apply`** until you are comfortable with permission prompts.

### Step 7 — Resume a session

```bash
drox-tui --list-sessions
drox-tui --workspace . --session ses_XXXXXXXX
```

**Stuck?** Run `/doctor`, restart Ollama, or open an [Issue](https://github.com/DroxKiwi/Drox---TUI---OR/issues).

---

## EN — Overview

Install **`drox-tui`**, run **Ollama**, open your project in a terminal. Each message runs a **local agent loop**: the TUI calls your model, executes tools (files, bash, grep, web with permission), and streams the transcript.

| Piece | Role |
|-------|------|
| **Ollama** | Local inference — your chosen model |
| **drox-tui** | Agent loop, terminal UI, permissions, sessions |
| **You** | Repo, model, `--plan` / `--apply`, allow/ask/deny rules |

No mandatory KDDS cloud. Session data in **`~/.drox/`** and **`<workspace>/.drox/`**.

---

## EN — Launch & install

```bash
cd /path/to/your-project
drox-tui --workspace .
```

**Windows**: download `drox-tui-2.0.5-windows-x64-setup.exe` from [Releases](https://github.com/DroxKiwi/Drox---TUI---OR/releases/tag/v2.0.5).

**Linux**: download `drox-tui-2.0.5-linux-x64.tar.gz` (if published), run `./install.sh`.

First launch: configure Ollama via **`Ctrl+Shift+L`** or **`/server`**.

| Option | Role |
|---|---|
| `--workspace` | Agent working directory |
| `--apply` | Allow real file writes |
| `--plan` | Plan mode |
| `--session ses_…` | Resume session |
| `-v` | Logs to `~/.drox/tui.log` |

---

## EN — Architecture

Unlike Drox IDE (Electron + `drox.exe` + RPC shim), the TUI **embeds** the **1.5.0** agent loop directly — **`tui_mono`**, no IDE layer.

Each send runs until **`[phase: done]`**. Phases: `analyzing` → `reading` → optional `clarifying` / `planning` → `acting` → `testing` / `verifying` → `answering` → **`done`**.

---

## EN — Features & shortcuts

- Visual diff: `/diff`, `/diff --stat`, `/diff <file>`, **`e`**, end-of-run banner
- Streaming transcript, markdown, tools, collapsible phases
- Composer: vim, `@file`, smart paste, images (Ollama)
- Permission modals with previews; sessions: resume, export, rewind
- Themes, keybindings, FR/EN i18n

| Shortcut | Action |
|---|---|
| `Ctrl+Shift+L` | AI server connection |
| `Ctrl+Shift+W` | Change workspace |
| `e` | Expand diff / tool output |
| `/update on` | Enable update checks (opt-in) |

Without `--apply`, file writes are **previewed** only.

---

## EN — Updates

Update checks are **opt-in** (`/update on`). When enabled, the client reads `releases/latest.json` — the **only** Drox product communication channel. [Releases](https://github.com/DroxKiwi/Drox---TUI---OR/releases).

---

## EN — What the product is not

| Not | Detail |
|-----|--------|
| Production agent terminal | Still experimental — solid 2.0.x foundation |
| Graphical IDE | See [Drox IDE](https://github.com/DroxKiwi/Drox---IDE---OR) |
| Mandatory Drox cloud | LLM and data stay local |
| Signed installer | Planned **2.0.7** |
| Open engine source | — |

---

## Liens / Links

| FR | EN |
|----|-----|
| [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR/releases) | Binaires officiels |
| [RELEASE_NOTES 2.0.5](docs/2.0.5/RELEASE_NOTES.md) | Notes de version |
| [Plan 2.0.5](docs/2.0.5/README.md) | Connexions LLM (livrée) |
| [Plan 2.0.6](docs/2.0.6/README.md) | Multi-pane (à venir) |
| [Plan 2.0.7](docs/2.0.7/README.md) | Signing & Linux |
| [Drox IDE OR](https://github.com/DroxKiwi/Drox---IDE---OR) | Produit sœur (éditeur) |

---

## Licence

MIT — voir le fichier `LICENSE` fourni avec l’archive d’installation.

---

*KDDS — Drox TUI. Moteur agent dérivé de Drox IDE 1.5.0.*
