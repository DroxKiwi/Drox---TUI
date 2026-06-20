//! Catalogue français (P0).

use super::keys::*;

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
            "Modifier : `/theme` · `/color` · `/vim` · `/language fr|en` · `/settings animations on|off` · `/settings mouse on|off` · Ctrl+Shift+L ou `/server` · Ctrl+Shift+W ou `/workspace` · `/onboarding`",
        ),
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
        _ => None,
    }
}
