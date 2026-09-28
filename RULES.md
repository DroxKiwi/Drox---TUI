# Règles Drox TUI (fork / dépôt public)

## 1. Commits

- Ne créer un **commit** que si l’utilisateur le demande explicitement.
- **Ne jamais** ajouter de trailer `Co-authored-by: Cursor <cursoragent@cursor.com>` (ni variante).
- Les agents IA commitent **uniquement** sous l’identité git locale du mainteneur.
- Langue des messages de commit : **français**.

## 2. README

- Français uniquement, première personne du singulier pour la voix auteur.
- But, historique, technique, remerciements ([Salesky](https://www.salesky.fr/)).
- Binaires via Releases de **ce** dépôt.

## 3. Secrets

- Ne pas committer `.env`, clés API, tokens, profils utilisateur (`.drox/`).
