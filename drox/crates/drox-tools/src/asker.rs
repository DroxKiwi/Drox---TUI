//! Mécanisme générique pour qu'un tool puisse poser une question à l'humain.
//!
//! L'implémentation concrète varie selon le client :
//! - `drox-cli` fournit un asker basé sur stdin/stdout (réponse texte libre
//!   ou choix numéroté).
//! - L'extension VS Code (Phase 2) fournira un asker qui ouvre une boîte de
//!   dialogue native.
//!
//! Le moteur (`drox-engine`) et les tools ne connaissent jamais l'UI : ils
//! manipulent uniquement le trait.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::ToolError;

/// Question posée à l'humain par un tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuestion {
    /// Identifiant logique de la question (facultatif), reflété tel quel dans
    /// la réponse correspondante quand plusieurs questions sont posées d'un
    /// coup. Si absent, le client peut générer un id à partir de l'index.
    #[serde(default)]
    pub id: Option<String>,
    /// Texte de la question, en clair.
    pub prompt: String,
    /// Si non vide, l'humain doit choisir parmi cette liste.
    #[serde(default)]
    pub choices: Vec<String>,
    /// Permet de sélectionner plusieurs choix (séparés par virgule côté CLI).
    #[serde(default)]
    pub allow_multiple: bool,
    /// Autorise un complément en texte libre **en plus** des choix (sprint
    /// Questions bloquantes — §2.13 du backlog : Cursor expose un champ
    /// « Add more optional details » sous chaque question). Les askers
    /// mono-texte (stdin) acceptent toujours du texte libre et peuvent
    /// ignorer ce champ ; les askers UI (extension) s'en servent pour
    /// afficher / masquer la zone de saisie.
    #[serde(default)]
    pub allow_free_text: bool,
}

/// Réponse fournie par l'humain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAnswer {
    /// Identifiant logique de la question répondue (recopié depuis
    /// `UserQuestion::id` si présent). `None` pour la rétrocompat
    /// mono-question.
    #[serde(default)]
    pub id: Option<String>,
    /// Texte brut ou choix sélectionnés joints par `,`. Toujours présent.
    pub text: String,
    /// Si la question proposait des choix, indices retenus (0-based).
    #[serde(default)]
    pub indices: Vec<usize>,
    /// Vrai si l'utilisateur a explicitement **skipé** cette question (touche
    /// Esc / bouton Ignorer côté UI). Le `text` est alors vide et `indices`
    /// vides. Le tool transmet l'info au modèle pour qu'il en tienne compte.
    #[serde(default)]
    pub skipped: bool,
}

/// Contrat permettant à un tool de solliciter l'humain.
///
/// `Send + Sync` car le tool peut être exécuté depuis n'importe quel runtime
/// task.
#[async_trait]
pub trait UserAsker: Send + Sync {
    /// Pose une question et attend une réponse. Renvoie `ToolError::Interactive`
    /// si l'humain refuse / annule.
    async fn ask(&self, question: UserQuestion) -> Result<UserAnswer, ToolError>;

    /// Pose **plusieurs** questions en un seul aller-retour client (Sprint
    /// Questions bloquantes — §2.13 du backlog). Implémentation par défaut :
    /// boucle séquentielle sur `ask`, ce qui reste fonctionnel pour les
    /// askers mono-question (stdin). Les askers structurés (JSON-RPC,
    /// extension VS Code) peuvent override pour grouper la file 1/N dans
    /// une seule carte UI.
    ///
    /// `title` est un titre optionnel pour la carte (groupage visuel).
    async fn ask_many(
        &self,
        questions: Vec<UserQuestion>,
        _title: Option<String>,
    ) -> Result<Vec<UserAnswer>, ToolError> {
        let mut answers = Vec::with_capacity(questions.len());
        for q in questions {
            let id = q.id.clone();
            let mut a = self.ask(q).await?;
            if a.id.is_none() {
                a.id = id;
            }
            answers.push(a);
        }
        Ok(answers)
    }
}
