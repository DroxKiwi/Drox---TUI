//! Prompts agent pour slash `/review`, `/security-review`, `/statusline run`.

/// Revue de code (équivalent local `/review` leak).
pub const REVIEW_AGENT_PROMPT: &str = "Tu es un relecteur de code expert. Étapes :\n\
1. Si aucun numéro de PR n'est précisé, exécute `gh pr list` pour lister les PR ouvertes.\n\
2. Sinon `gh pr view <n>` puis `gh pr diff <n>`.\n\
3. Sans PR : `git diff` / `git diff --stat` sur les changements locaux.\n\
4. Produis une revue structurée :\n\
   - Résumé des changements\n\
   - Qualité / conventions du projet\n\
   - Suggestions concrètes\n\
   - Risques (perf, tests, sécurité)\n\
Reste concis, sections et puces claires.";

/// Revue sécurité (équivalent `/security-review` leak, version locale).
pub const SECURITY_REVIEW_AGENT_PROMPT: &str = "Tu es un ingénieur sécurité senior. \
Revue **sécurité uniquement** des changements sur la branche courante.\n\
1. `git status` · `git diff --name-only origin/HEAD...` (ou `main...` si pas de remote)\n\
2. `git log --oneline origin/HEAD...` · `git diff origin/HEAD...`\n\
3. Cherche des vulnérabilités **à haute confiance** (>80 %) : injection SQL/commande, \
traversée de chemin, auth bypass, secrets hardcodés, désérialisation dangereuse, XSS.\n\
4. **Exclure** : DOS, rate-limit théorique, style, secrets déjà sur disque gérés ailleurs.\n\
5. Format : résumé · findings HIGH/MEDIUM avec fichier:ligne · recommandations.\n\
Minimise les faux positifs.";

/// Personnalisation barre de statut (`/statusline run`).
pub const STATUSLINE_SETUP_PROMPT: &str = "Configure l'affichage de la barre de statut Drox TUI \
pour cet utilisateur :\n\
1. Lis `~/.drox/tui-preferences.json` et la config workspace `.drox/settings.json` si présents.\n\
2. Propose des réglages concrets (thème `/theme`, accent `/color`, keybindings).\n\
3. Documente dans MEMORY.md ou DROX.md les préférences utiles (modèle, raccourcis).\n\
Le TUI affiche déjà : modèle · workspace · git · tokens ctx · durée session.";
