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

- [ ] Module `i18n` + prefs `ui_locale`
- [ ] FR / EN modales P0
- [ ] `/language` ou settings picker
- [ ] Changement à chaud
- [ ] Tests clés FR/EN

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
- [ ] Session resume / export
- [ ] Permissions modales
- [ ] Certification locale inchangée (pas de phone home)

## Release (fin de ligne)

- [ ] Bump version 2.0.2
- [ ] Installateur OR Windows
- [ ] RELEASE_NOTES OR
