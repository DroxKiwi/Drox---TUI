# Plan i18n — FR / EN (ligne 2.0.2)

> Reprend et formalise [`docs/2.0.1/PLAN-I18N-EN.md`](../2.0.1/PLAN-I18N-EN.md).

**Statut** : livré (2.0.2)  
**Objectif** : interface TUI entièrement utilisable en **français** et **anglais**, avec choix utilisateur persistant.

---

## Principes

1. **Langue UI ≠ langue agent** — les prompts système / réponses LLM restent libres ; seules les chaînes TUI sont traduites.
2. **Défaut intelligent** — locale OS (`fr*` → FR, sinon EN) ; override dans `/settings`.
3. **Pas de reload** — changement de langue à chaud (re-render immédiat).
4. **Clés stables** — identifiants anglais snake_case ; traductions dans fichiers dédiés.

---

## Périmètre traduction (P0 → P1)

| Priorité | Zones |
|---|---|
| **P0** | Modales (`/server`, Question, Permission, workspace), onboarding, notices push |
| **P1** | Slash palette, `/help`, settings, status line, toasts |
| **P2** | Messages engine relayés tels quels (FR/EN selon moteur) — hors scope i18n TUI |
| **P3** | Chaînes tests / assert — restent en anglais |

---

## Architecture proposée

```text
drox/crates/drox-tui/src/i18n/
├── mod.rs           # t(key), set_locale, Locale enum
├── locale.rs        # Fr, En
├── catalog_fr.rs    # ou fr.toml embarqué
└── catalog_en.rs
```

### API cible

```rust
pub enum UiLocale { Fr, En }

pub fn t(key: &str) -> &'static str;
pub fn set_locale(locale: UiLocale);
pub fn locale_from_prefs(prefs: &TuiPreferences) -> UiLocale;
```

### Persistance

`TuiPreferences` :

```json
{
  "ui_locale": "fr",
  "theme": "drox"
}
```

Commande : `/language fr` | `/language en` | `/settings` picker.

---

## Format catalogues

Option A — **Rust maps** (simple, compile-time) :

```rust
// catalog_fr.rs
pub fn get(key: &str) -> Option<&'static str> {
    match key {
        "modal.server.title" => Some("Connexion IA"),
        "modal.server.test" => Some("Tester"),
        ...
    }
}
```

Option B — **Fluent** / **gettext** — si volume > ~200 chaînes (post-2.0.2).

**Décision 2.0.2** : Option A pour livrer vite ; migrer si besoin.

---

## Inventaire clés (extrait)

| Clé | FR | EN |
|---|---|---|
| `modal.server.title` | Connexion IA | AI server |
| `modal.server.test` | Tester | Test |
| `modal.server.save` | Enregistrer | Save |
| `modal.question.confirm` | Confirmer | Confirm |
| `settings.language` | Langue | Language |
| `status.connected` | Connecté | Connected |
| `status.offline` | Hors ligne | Offline |
| `theme.drox` | Drox (phosphore) | Drox (phosphor) |

Inventaire complet à générer via grep `format!` / strings littérales dans `drox-tui/src/widgets/` et `slash/`.

---

## Détection locale OS

| OS | Source |
|---|---|
| Windows | `GetUserDefaultLocaleName` ou var `LANG` |
| Linux | `LC_ALL` / `LANG` |
| Fallback | `En` |

---

## Tests

- `cargo test -p drox-tui i18n` — toutes les clés P0 existent en FR et EN.
- Test manuel : basculer FR↔EN dans settings, parcourir `/server` + modal Question.
- Pas de régression : clés manquantes → fallback EN + log debug.

---

## Planning

| Étape | Tâche |
|---|---|
| 1 | Inventorier strings UI (~grep) |
| 2 | Créer module `i18n` + `UiLocale` dans prefs |
| 3 | Migrer modales P0 |
| 4 | Migrer slash + settings P1 |
| 5 | Documenter dans charte + `/help` bilingue |

---

## Hors scope 2.0.2

- Traduction des prompts système agent (`drox-cli` prompts)
- RTL / langues autres que FR/EN
- i18n messages `drox-engine` / outils
