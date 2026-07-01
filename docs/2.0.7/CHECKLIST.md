# Checklist — Ligne 2.0.7

## Code signing Windows

- [ ] Certificat Authenticode (OV ou EV) commandé
- [ ] `sign-release.ps1` + Inno `SignTool`
- [ ] CI signature sur release Windows
- [ ] QA SmartScreen (VM propre)
- [ ] README OR section vérification Windows

## Linux confiance & QA

- [ ] Clé GPG release + `.asc` sur OR
- [ ] `publish-or.ps1` génère signatures GPG
- [ ] QA terminal Linux natif (≥ 1 environnement)
- [ ] QA WSL publish pipeline
- [ ] README OR : `gpg --verify` + `sha256sum -c`
- [ ] QA `gpg --verify` documentée

## Release

- [ ] Bump version 2.0.7
- [ ] RELEASE_NOTES OR
- [ ] Merge → `main`
