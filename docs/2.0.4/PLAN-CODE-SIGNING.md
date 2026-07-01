# Plan — Confiance installation Windows & Linux

> **Statut** : **reporté ligne 2.0.7** — voir [`docs/2.0.7/PLAN-CODE-SIGNING-LINUX.md`](../2.0.7/PLAN-CODE-SIGNING-LINUX.md).

> **Problème** : les installateurs Drox TUI sont distribués sans signature de code reconnue. Windows affiche « Éditeur inconnu » / SmartScreen ; Linux repose sur SHA256 publié mais sans chaîne de confiance GPG.

**Objectif (2.0.7)** : un utilisateur qui télécharge depuis [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR) voit un **éditeur identifié** (Windows) et peut **vérifier cryptographiquement** les binaires (Linux).

---

## État actuel

| Plateforme | Artefact | Protection |
|---|---|---|
| Windows | `drox-tui-*-windows-x64-setup.exe` (Inno Setup) | SHA256 dans `SHA256SUMS-windows.txt` + `latest.json` |
| Linux | `drox-tui-*-linux-x64.tar.gz` | SHA256 dans `SHA256SUMS-linux.txt` + `latest.json` |
| Installateur | `packaging/windows/drox-tui-setup.iss` | `AppPublisher=Drox` — **pas** de certificat Authenticode |

Le checksum protège l’**intégrité** après téléchargement ; il ne prouve pas l’**identité de l’éditeur** au moment du double-clic.

---

## Windows — Authenticode (priorité)

### Ce qu’il faut acheter

| Type | Usage | SmartScreen | Coût indicatif |
|---|---|---|---|
| **OV** (Organization Validation) | Signer `.exe`, installateur | Réputation à construire (semaines/mois) | ~200–400 €/an |
| **EV** (Extended Validation) | Idem + réputation immédiate accélérée | Meilleur départ | ~400–700 €/an |

Fournisseurs courants : DigiCert, Sectigo, SSL.com, Certum.

**Prérequis** :
- Entité légale ou nom commercial vérifiable (Drox / DroxKiwi)
- Domaine web ou page GitHub officielle
- Pour EV : organisation enregistrée, processus KYC plus strict

### Pipeline de signature

1. **Compiler** l’installateur (inchangé) :

```powershell
.\packaging\build-and-pack.ps1
```

2. **Signer** avec `signtool` (Windows SDK) :

```powershell
$Cert = "CN=Drox, O=..."   # ou thumbprint du certificat installé
signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 `
  /sha1 $Thumbprint `
  dist\drox-tui-2.0.4-windows-x64-setup.exe
signtool verify /pa dist\drox-tui-2.0.4-windows-x64-setup.exe
```

3. **Intégrer dans Inno Setup** (`drox-tui-setup.iss`) :

```ini
[Setup]
SignTool=signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /sha1 $p /d $q $f
SignedUninstaller=yes
```

4. **Signer aussi** `drox-tui.exe` dans le stage avant empaquetage (défense en profondeur).

### SmartScreen — attentes réalistes

- Première release signée OV : alerte possible encore quelques semaines.
- **EV** : confiance plus rapide mais pas magique.
- Actions complémentaires :
  - [Soumettre le fichier à Microsoft](https://www.microsoft.com/en-us/wdsi/filesubmission) si faux positif
  - Maintenir le **même** sujet de certificat entre releases
  - Éviter de changer le nom de fichier à chaque patch mineur sans raison

### Secrets CI

- Certificat exporté `.pfx` → secret GitHub `WINDOWS_SIGNING_PFX` + mot de passe `WINDOWS_SIGNING_PASSWORD`
- Job `windows-sign` après `build-and-pack.ps1` (runner `windows-latest`)
- **Ne jamais** committer le `.pfx`

---

## Linux — confiance

### Niveau 1 — GPG sur les sommes (2.0.4 minimum)

Publier en plus des SHA256 :

```text
SHA256SUMS-2.0.4-linux.txt
SHA256SUMS-2.0.4-linux.txt.asc   ← signature GPG
```

Commandes éditeur :

```bash
gpg --armor --detach-sign SHA256SUMS-2.0.4-linux.txt
```

Utilisateur :

```bash
gpg --verify SHA256SUMS-2.0.4-linux.txt.asc SHA256SUMS-2.0.4-linux.txt
sha256sum -c SHA256SUMS-2.0.4-linux.txt
```

- [ ] Générer clé GPG dédiée `Drox TUI Release <release@...>`
- [ ] Publier la clé publique sur OR (`packaging/keys/drox-tui-release.asc`)
- [ ] Documenter dans README OR
- [ ] Intégrer dans `publish-or.ps1` / workflow release

### Niveau 2 — Paquet `.deb` signé (optionnel, post-2.0.4)

- Repo APT privé avec `Release` + `InRelease` signé
- Nécessite infrastructure (Launchpad, Cloudsmith, ou self-hosted)
- Hors scope strict 2.0.4 sauf demande explicite

### Niveau 3 — Flatpak / Flathub

- Signature intégrée au modèle Flatpak
- Long terme ; pas prioritaire si `tar.gz` + GPG suffit

---

## `/update` et signatures

Le module `engine/update_install.rs` vérifie déjà **SHA256** depuis `latest.json`.

Évolution 2.0.4 (optionnelle) :
- Champ `signature` ou URL `.asc` dans `latest.json`
- Vérification GPG avant install Linux
- Windows : Authenticode vérifié par le OS au lancement de l’installateur

Référence plan antérieur : signature ed25519 de `latest.json` ([PLAN-UPDATE 2.0.1](../2.0.1/PLAN-UPDATE.md)) — peut être M2 après Authenticode.

---

## Jalons (→ 2.0.7)

| Jalon | Livrable |
|---|---|
| **S0** | Décision OV vs EV + achat certificat |
| **S1** | Script `packaging/windows/sign-release.ps1` + doc |
| **S2** | Inno `SignTool` activé en release |
| **S3** | CI signe sur tag `v*` |
| **S4** | Clé GPG release + `.asc` sur OR |
| **S5** | README OR : « Vérifier l’éditeur » (Windows + Linux) |
| **S6** | QA : VM Windows propre, Ubuntu 24.04 |

---

## README utilisateur (extrait cible)

### Windows

> L’installateur est signé par **Drox** (certificat Authenticode). Clic droit → Propriétés → Signature numérique pour vérifier.

### Linux

> Vérifiez la signature GPG des sommes de contrôle avant d’extraire l’archive (voir `SHA256SUMS-*-linux.txt.asc`).

---

## Coûts et délais estimés

| Étape | Délai |
|---|---|
| Achat certificat OV | 1–5 jours ouvrés |
| Intégration pipeline | 1–2 jours dev |
| Réputation SmartScreen | Semaines (OV) / jours (EV) |
| Clé GPG + doc | 0,5 jour |

---

## Fichiers à modifier (implémentation)

| Fichier | Changement |
|---|---|
| `packaging/windows/drox-tui-setup.iss` | `SignTool`, `SignedUninstaller` |
| `packaging/windows/sign-release.ps1` | **nouveau** |
| `packaging/publish-or.ps1` | Appel sign + GPG |
| `.github/workflows/release.yml` | **nouveau** — sign on tag |
| `README.md` (OR) | Section confiance |
