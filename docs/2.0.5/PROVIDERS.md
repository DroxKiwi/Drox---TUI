# Prestataires LLM — références officielles & contrats connecteur

> Chaque connecteur **doit** suivre la doc officielle du prestataire. Les champs UI sont dérivés de ces specs — pas d’URL ou header « magique » non documenté.

---

## Self-hosted

### Ollama (local)

| | |
|---|---|
| **Doc** | [Ollama API](https://github.com/ollama/ollama/blob/main/docs/api.md) |
| **URL défaut** | `http://127.0.0.1:11434` |
| **Auth** | Aucune (local) ; reverse-proxy → headers custom |
| **Liste modèles** | `GET /api/tags` |
| **Chat / generate** | API Ollama native (`drox-llm` Ollama client) |
| **Headers custom** | `extra_headers` mergés sur chaque requête |

### vLLM

| | |
|---|---|
| **Doc** | [vLLM OpenAI-compatible server](https://docs.vllm.ai/en/latest/serving/openai_compatible_server.html) |
| **URL défaut** | `http://127.0.0.1:8000/v1` |
| **Auth** | Souvent `Authorization: Bearer` (optionnel en local) |
| **Liste modèles** | `GET /v1/models` |
| **Client** | OpenAI-compatible (`drox-llm`) |

### LM Studio

| | |
|---|---|
| **Doc** | [LM Studio — OpenAI compatible API](https://lmstudio.ai/docs/app/api/endpoints/openai) |
| **URL défaut** | `http://127.0.0.1:1234/v1` |
| **Auth** | Selon config locale (souvent aucune) |
| **Client** | OpenAI-compatible |

### OpenAI-compatible (générique)

| | |
|---|---|
| **Doc** | [OpenAI API reference](https://platform.openai.com/docs/api-reference) (schéma de référence) |
| **URL** | Saisie utilisateur (doit finir par `/v1` si serveur OpenAI-like) |
| **Auth** | Bearer ou header custom |
| **Headers custom** | Obligatoire pour gateways (nginx, Cloudflare Access, etc.) |

---

## Cloud

### Ollama Cloud

| | |
|---|---|
| **Doc** | [Ollama Cloud](https://ollama.com/cloud) · [API](https://github.com/ollama/ollama/blob/main/docs/api.md) |
| **URL** | `https://ollama.com` (ou endpoint documenté) |
| **Auth** | `Authorization: Bearer <API key>` |
| **Champs UI** | API key (masqué), modèle (liste après test) |
| **Connecteur** | API Ollama native + Bearer — **pas** OpenAI-compat sauf si doc l’indique |

### Mistral

| | |
|---|---|
| **Doc** | [Mistral AI — API](https://docs.mistral.ai/api/) · [Chat completions](https://docs.mistral.ai/api/#tag/chat) |
| **URL** | `https://api.mistral.ai/v1` |
| **Auth** | `Authorization: Bearer <MISTRAL_API_KEY>` |
| **Champs UI** | API key, modèle (ex. `mistral-small-latest`, `codestral-latest`) |
| **Connecteur** | OpenAI-compatible (`/v1/chat/completions`) |

### OVHcloud

| | |
|---|---|
| **Doc** | [AI Endpoints](https://help.ovhcloud.com/csm/en-gb-public-cloud-ai-machine-learning-endpoints) · [API](https://help.ovhcloud.com/csm/en-gb-public-cloud-ai-endpoints-getting-started) |
| **URL** | Endpoint régional documenté (ex. `https://…endpoints.ai.cloud.ovh.net/…`) |
| **Auth** | Token / header selon doc OVH (souvent Bearer ou clé dédiée) |
| **Champs UI** | Token, URL endpoint (ou sélecteur région + modèle), modèle déployé |
| **Connecteur** | OpenAI-compatible **si** endpoint exposé ainsi — valider par doc produit |

### Hugging Face

| | |
|---|---|
| **Doc** | [Inference Providers](https://huggingface.co/docs/inference-providers/index) · [Serverless Inference API](https://huggingface.co/docs/api-inference/index) |
| **URL** | `https://api-inference.huggingface.co` ou router HF (selon offre) |
| **Auth** | `Authorization: Bearer <HF_TOKEN>` |
| **Champs UI** | Token HF, modèle (`org/model`), option provider (si router) |
| **Connecteur** | Client dédié ou OpenAI-compat selon endpoint choisi — **test obligatoire** |

### Scaleway

| | |
|---|---|
| **Doc** | [Generative APIs](https://www.scaleway.com/en/docs/generative-apis/) · [API reference](https://www.scaleway.com/en/docs/generative-apis/reference-content/) |
| **URL** | `https://api.scaleway.ai/v1` (vérifier doc à jour) |
| **Auth** | `X-Auth-Token` ou `Authorization: Bearer` selon doc Scaleway |
| **Champs UI** | Secret key, modèle, région si applicable |
| **Connecteur** | OpenAI-compatible |

---

## Matrice auth → headers HTTP

| Mode | Header typique | Où configurer |
|---|---|---|
| Aucune | — | Self-hosted local |
| Bearer | `Authorization: Bearer <token>` | Cloud, vLLM distant |
| API key header | `Authorization` ou `X-Api-Key` | Selon prestataire |
| Custom | Table `extra_headers` | Self-hosted derrière proxy, cas avancés |

`AuthConfig::CustomHeaders` : **tous** les headers viennent de `extra_headers` (pas d’injection auto).

---

## Critères connecteur (par prestataire)

1. **Probe** (`Tester`) : liste modèles ou ping documenté → message d’erreur lisible si auth/URL incorrects.
2. **Chat agent** : un run minimal avec outils désactivés passe.
3. **Headers** : capture test HTTP (mock ou log dev) prouve les headers attendus.
4. **Doc** : lien doc officielle dans l’UI (aide contextuelle) + entrée dans ce fichier.

---

## Références code actuelles

| Fichier | Rôle |
|---|---|
| `drox-tui/src/engine/connection_library.rs` | Profils, presets, `profile_to_llm_config` |
| `drox-tui/src/app/state.rs` | `DeploymentKind`, `CloudProviderChoice` (à étendre) |
| `drox-tui/src/widgets/ai_server_dialog.rs` | Wizard connexion |
| `drox-tui/src/engine/llm_connection.rs` | Probe actuel |
| `drox-llm/src/config.rs` | `headers`, clients Ollama / OpenAI |
