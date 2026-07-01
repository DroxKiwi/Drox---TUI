# Plan — Code signing & préparation Linux (ligne 2.0.7)

> **Reporté depuis** [`docs/2.0.4/PLAN-CODE-SIGNING.md`](../2.0.4/PLAN-CODE-SIGNING.md) (plan archivé, non livré en 2.0.4).

**Objectif** : un utilisateur qui télécharge depuis [Drox---TUI---OR](https://github.com/DroxKiwi/Drox---TUI---OR) voit un **éditeur identifié** (Windows) et peut **vérifier cryptographiquement** les binaires (Linux GPG).

---

## Prérequis déjà livrés (2.0.3)

| Élément | Statut |
|---|---|
| Archive Linux `tar.gz` + `install.sh` | ✅ |
| `publish-or.ps1` + `latest.json` | ✅ |
| CI `ubuntu-latest` (tests) | ✅ |
| SHA256 Windows + Linux | ✅ |

La 2.0.7 **ajoute** signing et QA Linux approfondie — pas le packaging minimal.

---

## Windows — Authenticode

### Achat certificat

| Type | SmartScreen | Coût indicatif |
|---|---|---|
| **OV** | Réputation à construire | ~200–400 €/an |
| **EV** | Confiance accélérée | ~400–700 €/an |

### Pipeline

1. `packaging/build-and-pack.ps1` (inchangé)
2. **Nouveau** `packaging/windows/sign-release.ps1` — `signtool sign` + verify
3. `drox-tui-setup.iss` — `SignTool`, `SignedUninstaller=yes`
4. Signer `drox-tui.exe` avant empaquetage
5. CI : secrets `WINDOWS_SIGNING_PFX` + `WINDOWS_SIGNING_PASSWORD` sur tag `v*`

### SmartScreen

- Soumission Microsoft si faux positif
- Même sujet de certificat entre releases
- Doc README OR « Vérifier l’éditeur »

---

## Linux — confiance & QA

### Niveau 1 — GPG (priorité)

```text
SHA256SUMS-{version}-linux.txt
SHA256SUMS-{version}-linux.txt.asc
```

- [ ] Clé GPG `Drox TUI Release`
- [ ] `packaging/keys/drox-tui-release.asc` sur OR
- [ ] Intégration `publish-or.ps1`
- [ ] Section README OR : `gpg --verify` + `sha256sum -c`

### Niveau 2 — QA multi-environnement

- [ ] QA terminal Linux natif (≥ 1 distro, ex. Ubuntu 24.04)
- [ ] QA WSL2 depuis Windows (publish-or)
- [ ] Documenter écarts connus (presse-papiers image, etc.)
- [ ] Option CI `publish-linux-or.yml` renforcé

### Hors scope 2.0.7

- `.deb` / AppImage signé, Flatpak, notarisation macOS

---

## `/update` et signatures

- Aujourd’hui : SHA256 via `latest.json` (`update_install.rs`)
- 2.0.7 optionnel : URL `.asc` dans `latest.json`, vérif GPG avant install Linux
- Windows : confiance OS via installateur Authenticode

---

## Jalons

| Jalon | Livrable |
|---|---|
| **S0** | Décision OV vs EV + achat certificat |
| **S1** | `sign-release.ps1` + doc BUILD |
| **S2** | Inno SignTool en release |
| **S3** | CI signe sur tag |
| **S4** | Clé GPG + `.asc` sur OR |
| **S5** | README OR vérification éditeur |
| **S6** | QA VM Windows propre + Linux natif |

---

## Fichiers cibles

| Fichier | Changement |
|---|---|
| `packaging/windows/sign-release.ps1` | **nouveau** |
| `packaging/windows/drox-tui-setup.iss` | SignTool |
| `packaging/publish-or.ps1` | sign + GPG |
| `.github/workflows/release-sign.yml` | **nouveau** |
| `README.md` (OR) | Section confiance |
