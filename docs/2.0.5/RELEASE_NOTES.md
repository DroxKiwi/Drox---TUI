# Drox TUI v2.0.5 — Notes de release

**Produit** : Drox TUI `2.0.5`  
**Moteur** : dérivé du moteur agent Drox IDE `1.5.0`  
**Ligne** : Connexions LLM self-hosted & cloud  
**Certification** : local-first — pas de télémétrie Drox, pas de cloud obligatoire.

---

## Nouveautés

### Connexions LLM (`/server`, `Ctrl+Shift+L`)

- **Self-hosted** : Ollama local, vLLM, LM Studio, OpenAI-compatible, personnalisé — presets URL + auth
- **Cloud** : Ollama Cloud, Mistral, OVHcloud AI Endpoints, Hugging Face, Scaleway — presets dédiés
- **Auth corrigée** : Ollama Cloud et prestataires cloud utilisent `Authorization: Bearer` (plus de `x-api-key` erroné → 401)
- **Wizard** : choix prestataire → URL / token → test → sélection modèle ; lien **doc officielle** à l’étape connexion
- **Headers custom** : table clé/valeur (self-hosted, proxies, gateways)
- **Probe** : messages d’erreur actionnables (401 token, 403 droits, 404 URL)
- **Migration** : profils cloud existants corrigés automatiquement au chargement des préférences
- **`/doctor`** : diagnostics connexion LLM améliorés

### Presets intégrés

| Preset | URL défaut |
|---|---|
| Ollama local | `http://127.0.0.1:11434` |
| Ollama Cloud | `https://ollama.com` |
| Mistral | `https://api.mistral.ai/v1` |
| OVHcloud | `https://endpoints.ai.cloud.ovh.net` (à adapter) |
| Hugging Face | `https://api-inference.huggingface.co` |
| Scaleway | `https://api.scaleway.ai/v1` |
| vLLM | `http://127.0.0.1:8000/v1` |
| LM Studio | `http://127.0.0.1:1234/v1` |

---

## Installation

### Windows x64

1. Téléchargez `drox-tui-2.0.5-windows-x64-setup.exe` depuis [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR/releases/tag/v2.0.5)
2. Lancez l'installateur, cochez « Ajouter au PATH »
3. Nouveau terminal :

```powershell
drox-tui --workspace C:\chemin\projet
```

### Linux x64

```bash
tar xzf drox-tui-2.0.5-linux-x64.tar.gz
cd drox-tui-2.0.5-linux-x64
./install.sh
drox-tui --workspace ~/projets/mon-repo
```

*(Archive Linux publiée si incluse dans cette release.)*

---

## Migration depuis 2.0.4

- Préférences `~/.drox/tui-preferences.json` conservées
- Profils Ollama Cloud avec auth `x-api-key` migrés vers **Bearer** au premier lancement
- Aucune action requise si la connexion fonctionnait déjà

**Configurer un cloud** : `/server` → Cloud → prestataire → coller la clé API → Tester → choisir le modèle.

---

## Raccourcis utiles

| Commande / touche | Action |
|---|---|
| `Ctrl+Shift+L` | Assistant connexion IA |
| `/server` | Idem — URL, auth, test, modèle |
| `/doctor` | Vérifier Ollama / connexion active |

---

## Artefacts OR

| Fichier | Description |
|---|---|
| `drox-tui-2.0.5-windows-x64-setup.exe` | Installateur Windows |
| `drox-tui-2.0.5-linux-x64.tar.gz` | Archive Linux (si publiée) |
| `SHA256SUMS-windows.txt` | Empreinte SHA256 Windows |
| `SHA256SUMS-linux.txt` | Empreinte SHA256 Linux |
| `releases/latest.json` | Métadonnées pour `/update` |

---

## QA

- Tests automatiques : `cargo test -p drox-tui` (200+ tests engine)
- Smoke : Ollama Cloud Bearer + wizard `/server`

---

## Reporté (lignes suivantes)

- **Poste multi-pane + diff inline fil** → [`2.0.6`](../2.0.6/README.md)
- **Code signing + GPG Linux** → [`2.0.7`](../2.0.7/README.md)
- **Keychain OS pour secrets** → piste ultérieure

---

*Date de publication : 2026-06-19*
