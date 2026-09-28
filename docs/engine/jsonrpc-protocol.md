# Protocole JSON-RPC (`drox --serve`)

Le **TUI n’utilise pas** ce protocole pour un run normal. Il est documenté ici parce que le même workspace construit `drox-cli`, consommé par **Drox IDE**.

## Transport

- Process : `drox --serve`
- Canal : **stdio**, une trame JSON par ligne (**NDJSON**)
- Style : JSON-RPC 2.0

## Méthodes utiles (aperçu)

| Sens | Méthode / notif | Rôle |
|------|-----------------|------|
| Client → moteur | `initialize` | Handshake, capabilities |
| Client → moteur | `agent.run` | Lancer un run (message, modèle, cwd, permissions…) |
| Moteur → client | `agent/event` | Stream (texte, tools, phases) |
| Moteur → client | `agent/done` | Fin de run |
| Moteur → client | `tool/exec` | Demande d’outil **remote** (IDE) |
| Client → moteur | résultat `tool/exec` | Retour outil IDE |

Implémentation : `drox/crates/drox-cli/src/jsonrpc/`.

## Lien TUI

Même `drive_inner` ; seul le **front** change.  
Référence longue côté IDE : [docs/engine/jsonrpc-protocol.md](https://github.com/DroxKiwi/Drox---IDE/blob/main/docs/engine/jsonrpc-protocol.md).
