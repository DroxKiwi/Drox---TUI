# Checklist — Phase UI 2.0.2

## Charte graphique

- [x] Thème `drox` défaut — palette noir / vert phosphore
- [x] Variante `drox-ansi` 16 couleurs
- [x] Doc normative portable — [docs/THEME.md](../THEME.md)
- [x] Header statut connexion (pastille)
- [x] Cadres modales style Pip-Boy (coins arrondis)
- [x] Composer : curseur block + bordure pulse

## Souris

- [x] Capture souris activée (crossterm)
- [x] Scroll fil + modales molette
- [x] Clic boutons modales (Oui/Non/Tester)
- [x] Clic sélection profil `/server`
- [x] Hover sur lignes outil
- [x] `/settings mouse: off` désactive proprement
- [x] Parité clavier conservée

## Animations

- [x] `cursor-blink` composer
- [x] `phosphor-pulse` bordure run actif
- [x] `modal-in` apparition modales
- [x] `handshake` test connexion
- [x] `boot-splash` lancement TUI (dissolve pixelisé)
- [x] `/settings animations: off`

## i18n

- [x] Module `i18n` + prefs `ui_locale`
- [x] FR / EN modales P0
- [x] `/language` ou `/settings language`
- [x] Changement à chaud
- [x] Tests clés FR/EN
- [x] P1 : slash palette, `/help`, toasts, status lines
- [x] Modale `/settings` (langue, animations, souris, vim)

## Connexions LLM

- [x] Type `ConnectionProfile` persisté
- [x] Presets : Ollama local, Ollama Cloud, vLLM, custom
- [x] Headers custom + Bearer / ApiKeyHeader
- [x] Migration prefs 2.0.1
- [x] Modal `/server` refonte (assistant 3 étapes + persistance connexion validée)
- [x] Test connexion par profil
- [x] Charte Drox — thème de base + modal `/server`

## Non-régression

- [x] `cargo test -p drox-tui`
- [x] Session resume / export (QA partielle)
- [x] Permissions modales (QA partielle)
- [x] Certification locale inchangée (pas de phone home)

## Release (fin de ligne)

- [x] Bump version 2.0.2 (workspace Cargo)
- [ ] Installateur OR Windows (`packaging/build-and-pack.ps1` + push OR)
- [x] RELEASE_NOTES — [RELEASE_NOTES.md](RELEASE_NOTES.md)
