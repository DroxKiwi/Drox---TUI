//! Catalogue français (P0 + P1).

use super::keys::*;
use super::keys_p1::*;
use super::keys_update::*;

#[must_use]
pub fn get(key: &str) -> Option<&'static str> {
    match key {
        BOOT_TAGLINE => Some("terminal agent"),
        BOOT_INIT => Some("initialisation…"),
        STATUS_CONNECTED => Some("CONNECTE"),
        STATUS_NOT_CONFIGURED => Some("NON CONFIG"),
        STATUS_PLAN => Some("PLAN"),
        HEADER_QUIT_HINT => Some("Ctrl+Q quitter"),
        MODAL_PERMISSION => Some("Permission"),
        MODAL_PERMISSION_QUEUE => Some("Permission (+{} en attente)"),
        MODAL_QUESTION => Some("Question"),
        MODAL_QUESTION_QUEUE => Some("Question (+{} en attente)"),
        MODAL_FOOTER_FREE_TEXT => Some("Réponse libre — Entrée valider · Esc ignorer"),
        MODAL_FOOTER_CHOOSE_YN => Some("↑↓ choisir · Entrée valider · y/n · Esc ignorer"),
        MODAL_FOOTER_CHOOSE => Some("↑↓ choisir · Entrée valider · Esc ignorer"),
        MODAL_SCROLL_HINT => Some(" · PgUp/PgDn contenu"),
        MODAL_SERVER_TITLE => Some("CONNEXION SERVEUR IA"),
        MODAL_SERVER_FRAME => Some("/server · Ctrl+Shift+L"),
        MODAL_SERVER_STEP_DEPLOY => Some("Etape 1/3 — Perso ou cloud"),
        MODAL_SERVER_STEP_ENGINE => Some("Etape 2/3 — Moteur d'inference"),
        MODAL_SERVER_STEP_CLOUD => Some("Etape 2/3 — Prestataire cloud"),
        MODAL_SERVER_STEP_CONFIGURE => Some("Etape 3/3 — Connexion"),
        MODAL_SERVER_STEP_TESTING => Some("Test connexion"),
        MODAL_SERVER_STEP_MODEL => Some("Configuration modele"),
        MODAL_SERVER_STEP_RESET => Some("Nouvelle configuration"),
        MODAL_SERVER_FIELD_URL => Some("URL"),
        MODAL_SERVER_FIELD_AUTH => Some("Auth"),
        MODAL_SERVER_FIELD_HEADER_NAME => Some("Nom header"),
        MODAL_SERVER_FIELD_TOKEN => Some("Token / cle"),
        MODAL_SERVER_FIELD_EXTRA_HEADERS => Some("Headers supplementaires:"),
        MODAL_SERVER_FIELD_HEADER_PLUS_NAME => Some("Header + nom"),
        MODAL_SERVER_FIELD_HEADER_PLUS_VALUE => Some("Header + valeur"),
        MODAL_SERVER_FIELD_CONTEXT_MAX => Some("Context max"),
        MODAL_SERVER_FIELD_MAX_ITER => Some("Max iterations"),
        MODAL_SERVER_MODELS_TITLE => Some("Modeles disponibles"),
        MODAL_SERVER_MODELS_TOTAL => Some("… {} modele(s) au total"),
        MODAL_SERVER_BTN_ADD_HEADER => Some("[+] Ajouter header"),
        MODAL_SERVER_BTN_TEST => Some(" Tester connexion "),
        MODAL_SERVER_BTN_TESTING => Some(" Tester connexion… "),
        MODAL_SERVER_BTN_BACK => Some(" <- Retour "),
        MODAL_SERVER_BTN_NEW_CONFIG => Some(" Nouvelle configuration "),
        MODAL_SERVER_BTN_CONFIRM => Some(" Confirmer "),
        MODAL_SERVER_BTN_CANCEL => Some(" <- Annuler (Esc) "),
        MODAL_SERVER_RESET_TITLE => Some("Recommencer la configuration ?"),
        MODAL_SERVER_RESET_LINE1 => {
            Some("L'assistant repartira de l'etape 1 (perso ou cloud).")
        }
        MODAL_SERVER_RESET_LINE2 => {
            Some("La connexion actuelle reste active tant que la nouvelle n'est pas validee.")
        }
        MODAL_SERVER_HINT_LIST => Some("fleches · Entree · Esc annuler/retour"),
        MODAL_SERVER_HINT_CONFIGURE => Some("Tab · <-/-> auth · Entree tester · Esc retour"),
        MODAL_SERVER_HINT_TESTING => Some("Patientez… · Esc annuler"),
        MODAL_SERVER_HINT_MODEL => Some("Tab · fleches · Entree appliquer · Esc retour connexion"),
        MODAL_SERVER_HINT_RESET => Some("Entree confirmer · Esc annuler"),
        MODAL_SERVER_CONN_FAILED_PREFIX => Some("Connexion echouee"),
        DEPLOYMENT_PERSONAL => Some("Personnel (serveur perso, NAS, localhost…)"),
        DEPLOYMENT_CLOUD => Some("Cloud (hébergeur managé)"),
        ENGINE_VLLM => Some("vLLM (OpenAI-compatible)"),
        ENGINE_LM_STUDIO => Some("LM Studio"),
        ENGINE_OPENAI_COMPAT => Some("OpenAI-compatible (autre)"),
        ENGINE_CUSTOM => Some("Personnalisé (API sur mesure)"),
        AUTH_NONE => Some("Aucune"),
        AUTH_BEARER => Some("Bearer (Authorization)"),
        AUTH_API_KEY_HEADER => Some("Header API (ex. x-api-key)"),
        MODAL_WORKSPACE_TITLE => Some("Changer de workspace"),
        MODAL_WORKSPACE_SUBTITLE => {
            Some("Nouvelle session · fil effacé · re-bootstrap moteur")
        }
        MODAL_WORKSPACE_FRAME => Some("/workspace · Ctrl+Shift+W"),
        MODAL_WORKSPACE_CURRENT_DIR => Some("Dossier courant : {}"),
        MODAL_WORKSPACE_EXPLORER => Some("Explorateur"),
        MODAL_WORKSPACE_ENTRIES_TOTAL => Some("… {} élément(s)"),
        MODAL_WORKSPACE_BTN_SELECT => Some("Choisir ce dossier"),
        MODAL_WORKSPACE_FIELD_PATH => Some("Chemin manuel"),
        MODAL_WORKSPACE_VALID => Some("Workspace valide"),
        MODAL_WORKSPACE_CONFIRM_WARN => {
            Some("Une nouvelle session sera créée. Le fil courant sera effacé.")
        }
        MODAL_WORKSPACE_HINT_EDIT => {
            Some("↑↓ · Entrée ouvrir · Retour arrière remonter · Tab · Ctrl+Entrée choisir")
        }
        MODAL_WORKSPACE_HINT_CONFIRM => Some("Entrée confirmer · Esc retour"),
        ONBOARDING_FRAME => Some("Onboarding"),
        ONBOARDING_TITLE => Some("Premier pas avec Drox"),
        ONBOARDING_STEP => Some("Étape {}/{}"),
        ONBOARDING_FOOTER_NEXT => Some("Entrée suivant · Esc passer"),
        ONBOARDING_FOOTER_DONE => Some("Entrée terminer · Esc passer"),
        ONBOARDING_STEP_0 => Some(
            "Connexion IA — Ctrl+Shift+L ou `/server` : adresse Ollama, test de connexion, choix du modèle. Obligatoire avant d'envoyer un message.",
        ),
        ONBOARDING_STEP_1 => Some(
            "Workspace — Ctrl+Shift+W ou `/workspace` : changer le dossier de travail (nouvelle session). `/add-dir` ajoute un dossier en plus pour la session.",
        ),
        ONBOARDING_STEP_2 => {
            Some("Bienvenue dans Drox TUI — REPL terminal branché sur le moteur Rust local.")
        }
        ONBOARDING_STEP_3 => Some(
            "Workspace — vérifiez le chemin dans le header. Sans --apply, les écritures fichier sont simulées.",
        ),
        ONBOARDING_STEP_4 => Some("Démarrage — /init puis /init run pour créer DROX.md et .drox/."),
        ONBOARDING_STEP_5 => {
            Some("Composer — Entrée envoie · Shift+Entrée nouvelle ligne · ! mode bash · @ fichiers.")
        }
        ONBOARDING_STEP_6 => Some(
            "Navigation — / palette · Ctrl+F fil · Ctrl+R historique · e viewer outil.",
        ),
        ONBOARDING_STEP_7 => Some("Sessions — /sessions · /resume ses_… · mémoire /search · /rewind."),
        ONBOARDING_STEP_8 => {
            Some("Personnalisation — /theme · /color · /vim · /keybindings init · Ctrl+Shift+L connexion IA.")
        }
        SETTINGS_TITLE => Some("Réglages TUI (`~/.drox/tui-preferences.json`)"),
        SETTINGS_FILE => Some("fichier"),
        SETTINGS_THEME => Some("thème"),
        SETTINGS_ACCENT => Some("accent session"),
        SETTINGS_ACCENT_DEFAULT => Some("défaut"),
        SETTINGS_VIM => Some("vim composer"),
        SETTINGS_ANIMATIONS => Some("animations UI"),
        SETTINGS_MOUSE => Some("souris"),
        SETTINGS_COPY_FULL => Some("/copy réponse complète"),
        SETTINGS_TITLE_RENAME => Some("titre terminal depuis /rename"),
        SETTINGS_ONBOARDING => Some("onboarding vu"),
        SETTINGS_LANGUAGE => Some("langue UI"),
        SETTINGS_HINTS => Some(
            "Modale : `/settings` · dump texte : `/settings print` · thème `/theme` · connexion Ctrl+Shift+L ou `/server`",
        ),
        SETTINGS_MODAL_FOOTER => {
            Some("↑↓ naviguer · Entrée/Espace/←→ modifier · Esc fermer")
        }
        SETTINGS_VALUE_ON => Some("oui"),
        SETTINGS_VALUE_OFF => Some("non"),
        SETTINGS_RECENT_WS => Some("workspaces récents"),
        SETTINGS_LLM_SECTION => Some("— Connexion IA"),
        SETTINGS_LLM_ENGINE => Some("moteur"),
        SETTINGS_LLM_SERVER => Some("serveur"),
        SETTINGS_LLM_MODEL => Some("modele"),
        SETTINGS_LLM_NUM_CTX => Some("num_ctx"),
        SETTINGS_LLM_MAX_ITER => Some("max_iterations"),
        LANGUAGE_USAGE => Some("Usage : `/language fr` ou `/language en`"),
        LANGUAGE_CHANGED => Some("Langue UI : {} ({})"),
        SERVER_STATUS_URL_REQUIRED => Some("URL serveur requise"),
        SERVER_STATUS_HEADER_REQUIRED => Some("Nom du header requis"),
        SERVER_STATUS_TESTING => Some("Test de connexion en cours…"),
        SERVER_STATUS_NO_MODELS => {
            Some("Serveur joignable mais aucun modele liste (pull / deploy…)")
        }
        SERVER_STATUS_CONN_FAILED => Some("Connexion echouee : {}"),
        SERVER_STATUS_CONN_OK => {
            Some("Connexion OK — {} modele(s) · configurez le modele ci-dessous")
        }
        SERVER_STATUS_TEST_CANCELLED => Some("Test annule — modifiez la connexion"),
        WORKSPACE_STATUS_PATH_REQUIRED => Some("Chemin requis"),
        WORKSPACE_STATUS_ALREADY_CURRENT => Some("Déjà le workspace courant"),
        WORKSPACE_STATUS_ENTER_CONFIRM => Some("Entrée pour confirmer · nouvelle session"),
        WORKSPACE_STATUS_HINT_NAV => {
            Some("↑↓ naviguer · Entrée ouvrir · Tab · Choisir ce dossier")
        }
        WORKSPACE_STATUS_PICK_FOLDER => {
            Some("Ouvrez un lecteur (Entrée) puis choisissez le dossier")
        }
        WORKSPACE_VALIDATE_PATH_REQUIRED => Some("chemin requis"),
        WORKSPACE_VALIDATE_NOT_FOUND => Some("chemin introuvable ou inaccessible : {} ({})"),
        WORKSPACE_VALIDATE_NOT_DIR => Some("{} n'est pas un répertoire"),
        WORKSPACE_VALIDATE_NOT_UTF8 => Some("chemin non UTF-8"),
        SLASH_PALETTE_HELP => Some("aide — liste des commandes"),
        SLASH_PALETTE_SERVER => Some("connexion IA Ollama (Ctrl+Shift+L)"),
        SLASH_PALETTE_WORKSPACE => Some("changer workspace (Ctrl+Shift+W)"),
        SLASH_PALETTE_CLEAR => Some("effacer le fil UI"),
        SLASH_PALETTE_STATUS => Some("workspace, modèle, session"),
        SLASH_PALETTE_CONTEXT => Some("tokens et marge contexte"),
        SLASH_PALETTE_COST => Some("usage tokens session"),
        SLASH_PALETTE_COMPACT => Some("compaction LLM transcript"),
        SLASH_PALETTE_MEMORY => Some("sessions archivées · search"),
        SLASH_PALETTE_SEARCH => Some("recherche mémoire longue"),
        SLASH_PALETTE_PERMISSIONS => Some("règles permission"),
        SLASH_PALETTE_PLAN => Some("mode plan"),
        SLASH_PALETTE_CONFIG => Some("réglages runtime"),
        SLASH_PALETTE_DOCTOR => Some("diagnostic environnement"),
        SLASH_PALETTE_HOOKS => Some("hooks Pre/Post tool"),
        SLASH_PALETTE_MCP => Some("serveurs MCP"),
        SLASH_PALETTE_SKILLS => Some("skills locaux"),
        SLASH_PALETTE_SESSION => Some("transcript courant"),
        SLASH_PALETTE_RENAME => Some("titre personnalisé session"),
        SLASH_PALETTE_SESSIONS => Some("lister sessions"),
        SLASH_PALETTE_RESUME => Some("reprendre ses_…"),
        SLASH_PALETTE_REWIND => Some("rembobiner transcript"),
        SLASH_PALETTE_COPY => Some("copier dernière réponse assistant"),
        SLASH_PALETTE_ADD_DIR => Some("répertoire de travail additionnel"),
        SLASH_PALETTE_EXPORT => Some("exporter conversation"),
        SLASH_PALETTE_DIFF => Some("git diff workspace"),
        SLASH_PALETTE_FILES => Some("fichiers vus dans le fil"),
        SLASH_PALETTE_BRANCH => Some("branche git"),
        SLASH_PALETTE_THEME => Some("palette couleurs TUI"),
        SLASH_PALETTE_COLOR => Some("accent session"),
        SLASH_PALETTE_VIM => Some("mode vim composer (Esc NORMAL/INSERT)"),
        SLASH_PALETTE_KEYBINDINGS => Some("raccourcis · init · reload"),
        SLASH_PALETTE_TERMINAL_SETUP => Some("guide terminal + keybindings"),
        SLASH_PALETTE_SETTINGS => Some("préférences TUI"),
        SLASH_PALETTE_LANGUAGE => Some("langue UI (fr|en)"),
        SLASH_PALETTE_ONBOARDING => Some("guide premier lancement"),
        SLASH_PALETTE_INIT => Some("scaffold workspace"),
        SLASH_PALETTE_SANDBOX => Some("état sandbox bash"),
        SLASH_PALETTE_REVIEW => Some("revue code (agent)"),
        SLASH_PALETTE_SECURITY_REVIEW => Some("revue sécurité (agent)"),
        SLASH_PALETTE_STATUSLINE => Some("barre de statut TUI"),
        SLASH_PALETTE_EXIT => Some("quitter"),
        SLASH_PALETTE_TITLE => Some("Commandes slash"),
        SLASH_PALETTE_FILTER => Some("Filtre : /{}"),
        SLASH_PALETTE_EMPTY => Some("Aucune commande correspondante"),
        SLASH_PALETTE_MORE => Some(" … +{} autres"),
        SLASH_PALETTE_FOOTER => Some("↑↓ choisir · Entrée insérer · Esc fermer"),
        SLASH_HELP_BODY => Some(
            "Commandes : /help /clear /exit /status /model /server /session /sessions \
/newsession /resume <id> /rename [/rename <titre>] /copy [/copy N] /add-dir <chemin> /workspace [/workspace <chemin>] /vim /settings /update [/update check|on|off|snooze|dismiss|install] /onboarding /compact /memory [/memory <slug>|search <q>] /search <q> /permissions /plan [/plan off] /context /hooks [/hooks reload] /config /doctor /mcp [/mcp tools|resources|ping] /skills [/skills <name>] /cost /stats /usage /branch /rewind /export [/export fichier] /theme [/theme dark] /color [/color cyan] /keybindings [/keybindings init] /diff /files /init [/init run] /terminal-setup /sandbox /review [/review PR] /security-review /statusline [/statusline run]",
        ),
        COMPOSER_HELP_TITLE => Some(" Aide composer (?) — Esc fermer "),
        COMPOSER_HELP_0 => Some("Ctrl+Shift+L  connexion IA (Ollama) · /server"),
        COMPOSER_HELP_1 => Some("Ctrl+Shift+W  changer workspace · /workspace"),
        COMPOSER_HELP_2 => Some("!          mode bash (shell sans agent)"),
        COMPOSER_HELP_3 => Some("/          commandes slash · /help liste complète"),
        COMPOSER_HELP_4 => Some("@          référence fichier · Tab compléter"),
        COMPOSER_HELP_5 => Some("/skills    complétion nom de skill"),
        COMPOSER_HELP_6 => Some("Tab        accepter suggestion · ↑↓ naviguer"),
        COMPOSER_HELP_7 => Some("Ctrl+R     historique prompts · Ctrl+F recherche fil"),
        COMPOSER_HELP_8 => Some("e          développer dernière sortie outil"),
        COMPOSER_HELP_9 => Some("Ctrl+V     collage texte · image (chemin ou presse-papiers Win)"),
        COMPOSER_HELP_10 => Some("/vim       mode vim composer (Esc INSERT/NORMAL)"),
        COMPOSER_HELP_11 => Some("Esc        annuler · Ctrl+Q quitter"),
        SLASH_MSG_CLEAR => Some("Fil effacé (transcript disque conservé)."),
        SLASH_MSG_RESUME_USAGE => Some("Usage : /resume ses_<uuid>"),
        SLASH_MSG_UNKNOWN_CMD => Some("Commande inconnue : {}. {}"),
        SLASH_MSG_MEMORY_SEARCH_USAGE => {
            Some("Usage : /search <mots-clés> — mémoire `.drox/memory/sessions/`")
        }
        SLASH_MSG_MEMORY_SEARCH_SHORT => Some("Usage : /memory search <mots-clés>"),
        SLASH_MSG_NO_SESSIONS => Some("Aucune session transcript (ses_*.jsonl)."),
        SLASH_MSG_SESSIONS_LIST_ERR => Some("Liste sessions : {}"),
        SLASH_MSG_COPY_USAGE => Some("Usage : /copy [N] — N=1 (dernier), 2, … Reçu : {}"),
        SLASH_MSG_RUN_BLOCKED => {
            Some("Impossible : un run est en cours. Annulez d'abord (Esc).")
        }
        SETTINGS_LLM_UNCONFIGURED => {
            Some("— Connexion IA : non configurée (Ctrl+Shift+L ou `/server`)")
        }
        SETTINGS_API_KEY_SET => Some("définie"),
        SETTINGS_API_KEY_ABSENT => Some("absente"),
        SETTINGS_PROFILES => Some("  profils LLM : {} (actif: {})"),
        STATUS_READY => {
            Some("Prêt — @fichier · Ctrl+F fil · Ctrl+R historique · ! bash · Entrée envoyer")
        }
        STATUS_AGENT_RUNNING => Some("Agent en cours…"),
        STATUS_REWIND_CANCELLED => Some("Rembobinage annulé"),
        STATUS_VIEWER_CLOSED => Some("Viewer outil fermé"),
        STATUS_COPY_CANCELLED => Some("Copie annulée"),
        STATUS_THEME_CANCELLED => Some("Sélection thème annulée"),
        STATUS_THEME_APPLIED => Some("Thème appliqué"),
        STATUS_BASH_EXITED => Some("Mode bash quitté"),
        STATUS_DISPLAY_TOGGLED => Some("Affichage {} basculé (e)"),
        STATUS_RUN_CANCELLED => Some("Run annulé (Ctrl+C)"),
        STATUS_RUN_ABORTED => Some("Annulé"),
        STATUS_RUN_ERROR => Some("Erreur — prêt"),
        STATUS_CTRL_C_QUIT => Some("Ctrl+C encore pour quitter"),
        STATUS_BASH_MODE => Some("Mode bash — commande sans agent"),
        STATUS_BASH_MODE_HINT => Some("Entrée exécuter · Esc quitter bash"),
        STATUS_THEME_PICKER => Some("/theme — choisir un thème"),
        STATUS_COMPACTION_RUNNING => Some("Compaction LLM en cours…"),
        STATUS_COMPACTION_DONE => Some("Compaction live terminée"),
        STATUS_COMPACTION_PREVIEW => Some("Aperçu compaction affiché"),
        STATUS_COMPACTION_FAILED => Some("Compaction échouée"),
        STATUS_LLM_SAVE_FAILED => Some("Échec enregistrement connexion IA"),
        STATUS_WORKSPACE_CHANGE_FAILED => Some("Échec changement workspace"),
        STATUS_DOCTOR_RUNNING => Some("Diagnostic en cours…"),
        STATUS_DOCTOR_DONE => Some("Diagnostic terminé"),
        STATUS_MCP_RUNNING => Some("MCP…"),
        STATUS_REWIND_PICKER => Some("/rewind — choisir un message"),
        STATUS_TRANSCRIPT_REWOUND => Some("Transcript rembobiné"),
        STATUS_EXPORT_DONE => Some("Export terminé"),
        STATUS_SESSION_RENAMED => Some("Titre session mis à jour"),
        STATUS_COPY_DONE => Some("Copie terminée"),
        STATUS_COPY_PICKER => Some("/copy — choisir le contenu"),
        STATUS_BASH_ERROR => Some("bash en erreur"),
        STATUS_AGENT_BUSY => Some("Impossible pendant un run agent — annulez d'abord (Esc)."),
        STATUS_AGENT_IDLE => Some("Prêt"),
        STATUS_TRANSCRIPT_SEARCH => Some("Recherche transcript (Ctrl+F)"),
        STATUS_SEARCH_CLOSED => Some("Recherche fermée"),
        STATUS_HISTORY_SEARCH => Some("Recherche historique (Ctrl+R)"),
        STATUS_HISTORY_CANCELLED => Some("Recherche historique annulée"),
        STATUS_HISTORY_ACCEPTED => Some("Historique accepté"),
        STATUS_SERVER_DIALOG => Some("/server — assistant connexion IA (Ctrl+Shift+L)"),
        STATUS_WORKSPACE_DIALOG => {
            Some("/workspace — changer le répertoire de travail (Ctrl+Shift+W)")
        }
        STATUS_WORKSPACE_CHANGED => Some("Workspace : {}"),
        STATUS_WORKSPACE_CANCELLED => Some("Changement workspace annulé"),
        STATUS_LLM_SAVED => Some("Connexion IA enregistree"),
        STATUS_LLM_CANCELLED => Some("Connexion IA annulée"),
        STATUS_SLASH_PALETTE => Some("Palette slash — /"),
        STATUS_SLASH_PALETTE_CLOSED => Some("Palette slash fermee"),
        STATUS_AT_REF_INSERTED => Some("Référence @{} insérée"),
        STATUS_SLASH_CMD_READY => Some("Commande {} — Entrée pour exécuter"),
        STATUS_SKILL_READY => Some("Skill {} — Entrée pour lire"),
        STATUS_BASH_MODE_FOOTER => Some("Mode bash — Entrée exécute · Esc quitte"),
        STATUS_IMAGE_PASTED => Some("Image collée — {}"),
        STATUS_IMAGE_PATH => Some("Image — {} → {}"),
        STATUS_PERMISSION_WAITING => Some("Réponse requise"),
        STATUS_ANIMATIONS_ON => Some("Animations UI activées"),
        STATUS_ANIMATIONS_OFF => Some("Animations UI désactivées"),
        STATUS_MOUSE_ON => Some("Souris activée — scroll fil et clic modales"),
        STATUS_MOUSE_OFF => Some("Souris désactivée — clavier inchangé"),
        STATUS_THEME_MIGRATED => {
            Some("Thème Drox appliqué (charte 2.0.2) — /theme pour changer")
        }
        TOAST_LLM_REQUIRED => Some("Configurez Ollama pour envoyer des messages à l'agent"),
        TOAST_ASSISTANT_COPIED => Some("Réponse assistant copiée"),
        TOAST_CLIPBOARD => Some("Copié dans le presse-papiers"),
        TOAST_WORKSPACE_CHANGED => Some("Workspace changé · nouvelle session"),
        TOAST_LLM_SAVED => Some("Connexion IA enregistree"),
        TOAST_ONBOARDING_DONE => Some("Onboarding terminé — /onboarding pour revoir"),
        SYSTEM_CLIPBOARD_UNAVAILABLE => Some("Presse-papiers indisponible — utilisez /copy"),
        SYSTEM_COPY_NO_ASSISTANT => Some("Aucune réponse assistant à copier."),
        SLASH_PALETTE_UPDATE => Some("vérifier les mises à jour TUI"),
        UPDATE_HELP_BODY => Some(
            "Mises à jour TUI (opt-in) — /update check · on · off · snooze [jours] · dismiss · install",
        ),
        UPDATE_STATUS_HEADER => Some("— Mises à jour TUI —"),
        UPDATE_STATUS_VERSION => Some("Version installée : {}"),
        UPDATE_STATUS_ENABLED => Some("Vérification : activée (aucune requête auto sans consentement)"),
        UPDATE_STATUS_DISABLED => Some("Vérification : désactivée — /update on pour activer"),
        UPDATE_STATUS_DISMISSED => Some("Version ignorée : {}"),
        UPDATE_STATUS_SNOOZE => Some("Rappel reporté jusqu'à : {}"),
        UPDATE_STATUS_LAST_CHECK => Some("Dernière vérification : {}"),
        UPDATE_STATUS_NEVER_CHECKED => Some("Dernière vérification : jamais"),
        UPDATE_STATUS_HINTS => Some(
            "Sous-commandes : check · on · off · snooze [jours] · dismiss · install",
        ),
        UPDATE_ON => Some("Vérification des mises à jour activée."),
        UPDATE_OFF => Some("Vérification des mises à jour désactivée."),
        UPDATE_SNOOZE => Some("Rappel MAJ reporté de {} jour(s)."),
        UPDATE_DISMISS => Some("Notification MAJ ignorée pour la version courante."),
        UPDATE_CHECK_STUB => Some(
            "Vérification réseau non disponible — relancez /update check.",
        ),
        UPDATE_CHECK_RUNNING => Some("Vérification des mises à jour…"),
        UPDATE_CHECK_REMOTE => Some("Version distante : {}"),
        UPDATE_CHECK_UP_TO_DATE => Some("Vous êtes à jour."),
        UPDATE_CHECK_AVAILABLE => Some("Mise à jour {} disponible — /update install ou Ctrl+Shift+U."),
        UPDATE_CHECK_NEWER_LOCAL => Some(
            "Version locale ({}) plus récente que le dépôt OR ({}).",
        ),
        UPDATE_CHECK_FAILED => Some("Vérification MAJ échouée : {}"),
        UPDATE_CHECK_RELEASE_NOTES => Some("Notes : {}"),
        UPDATE_INSTALL_STUB => Some(
            "Installation automatique non disponible — téléchargez depuis GitHub Releases.",
        ),
        UPDATE_INSTALL_MODAL_TITLE => Some("Installer la mise à jour"),
        UPDATE_INSTALL_MODAL_BODY => Some("{} → {}"),
        UPDATE_INSTALL_MODAL_SHA => Some("SHA256 : {}"),
        UPDATE_INSTALL_MODAL_NOTES => Some("Notes : {}"),
        UPDATE_INSTALL_MODAL_FOOTER => {
            Some("Entrée = télécharger et installer · Esc = annuler")
        }
        UPDATE_INSTALL_RUNNING => Some("Téléchargement et vérification…"),
        UPDATE_INSTALL_LAUNCHED => Some("Installateur lancé — fermeture du TUI…"),
        UPDATE_INSTALL_SCHEDULED => Some("Mise à jour programmée — relance imminente…"),
        UPDATE_INSTALL_MANUAL => Some("{}"),
        UPDATE_INSTALL_FAILED => Some("Installation MAJ échouée : {}"),
        UPDATE_INSTALL_NO_RELEASE => Some("Aucun manifeste distant — lancez /update check."),
        UPDATE_INSTALL_NOT_AVAILABLE => Some("Aucune mise à jour disponible à installer."),
        UPDATE_INSTALL_UNSUPPORTED => Some("Aucun artefact pour cette plateforme dans latest.json."),
        UPDATE_SNOOZE_USAGE => Some("Usage : /update snooze <jours> — ex. /update snooze 7"),
        SETTINGS_UPDATE => Some("Mises à jour"),
        UPDATE_BANNER => Some(
            "Mise à jour {} disponible — Ctrl+Shift+U installer · u plus tard · /update",
        ),
        UPDATE_HEADER_PILL => Some("MAJ {}"),
        _ => None,
    }
}
