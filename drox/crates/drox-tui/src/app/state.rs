//! Configuration et état mutable de la session TUI.

use std::collections::HashSet;

use camino::Utf8PathBuf;
use drox_tools::UserQuestion;
use drox_types::ToolUseId;
use serde_json::Value;

use ratatui::text::Line;

use crate::view::{
    bash_progress, highlight_line, hook_progress, render, ActiveBashRun, ActiveHookProgress, LogEntry,
    PermissionPreview, ScrollViewerState, TranscriptSearchState,
};
use crate::ui::{resolve_palette, SessionAccent, ThemePalette, TuiThemeSetting};

/// Item affiché dans le panneau todos.
#[derive(Debug, Clone)]
pub struct TodoItemView {
    pub id: String,
    pub content: String,
    pub status: String,
}

/// Snapshot courant de la to-do list session.
#[derive(Debug, Clone)]
pub struct TodoSnapshot {
    pub items: Vec<TodoItemView>,
    pub summary: String,
}

/// Étape du plan de cours affichée dans le panneau.
#[derive(Debug, Clone)]
pub struct CourseStepView {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
}

/// Snapshot courant du plan de cours.
#[derive(Debug, Clone)]
pub struct CourseSnapshot {
    pub title: String,
    pub steps: Vec<CourseStepView>,
    pub summary: String,
}

/// Paramètres de démarrage (CLI / env).
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: String,
    pub model: String,
    pub workspace: Utf8PathBuf,
    pub apply: bool,
    pub plan_mode: bool,
    pub mode: Option<String>,
    pub allow: Vec<String>,
    pub ask: Vec<String>,
    pub deny: Vec<String>,
    pub no_settings: bool,
    pub max_iterations: usize,
    pub api_key: Option<String>,
    /// Fenetre de contexte LLM (`num_ctx` Ollama).
    pub num_ctx: i64,
    /// Reprend un transcript `ses_…` existant.
    pub session: Option<String>,
    /// Répertoire des transcripts (défaut `~/.drox/sessions`).
    pub session_dir: Option<Utf8PathBuf>,
}

/// Phase haute niveau de l'écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppPhase {
    #[default]
    Idle,
    Running,
    Prompt,
    Rewind,
    Theme,
    SlashPalette,
    Copy,
    Onboarding,
    AiServer,
    Workspace,
    Settings,
    UpdateInstall,
}

impl AppPhase {
    #[must_use]
    pub const fn is_modal(self) -> bool {
        matches!(
            self,
            Self::Prompt
                | Self::Rewind
                | Self::Theme
                | Self::SlashPalette
                | Self::Copy
                | Self::Onboarding
                | Self::AiServer
                | Self::Workspace
                | Self::Settings
                | Self::UpdateInstall
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComposerMode {
    #[default]
    Normal,
    Multiline,
    /// Mode bash intégré (`!`) — exécution shell sans agent.
    Bash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunStatus {
    #[default]
    None,
    Completed,
    Cancelled,
    Error,
}

/// Dialogue question / permission en cours.
#[derive(Debug, Clone)]
pub struct PromptDialog {
    pub question: UserQuestion,
    pub buffer: String,
    pub choice_index: usize,
    pub file_preview: Option<PermissionPreview>,
    /// Défilement du corps du message (PgUp/PgDn).
    pub body_scroll: u16,
}

/// Sélecteur `/rewind` — choix d'un message utilisateur.
#[derive(Debug, Clone)]
pub struct RewindDialog {
    pub choices: Vec<RewindChoiceView>,
    pub cursor: usize,
}

/// Assistant premier lancement (`/onboarding`).
#[derive(Debug, Clone, Default)]
pub struct OnboardingDialog {
    pub step: usize,
}

/// Étape du dialogue connexion IA (`/server`) — assistant 3 phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiServerStep {
    /// 1 — Perso ou cloud
    ChooseDeployment,
    /// 2 — Moteur d'inférence (perso)
    ChoosePersonalEngine,
    /// 2 — Prestataire (cloud)
    ChooseCloudProvider,
    /// 3 — URL, auth, headers
    ConfigureConnection,
    Testing,
    /// 3 — Après test OK : modèle + contexte
    SelectModel,
    /// Confirmation repartir de zéro
    ConfirmReset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentKind {
    Personal,
    Cloud,
}

impl DeploymentKind {
    pub const ALL: &'static [Self] = &[Self::Personal, Self::Cloud];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Personal => crate::i18n::t(crate::i18n::keys::DEPLOYMENT_PERSONAL),
            Self::Cloud => crate::i18n::t(crate::i18n::keys::DEPLOYMENT_CLOUD),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonalEngineChoice {
    Ollama,
    Vllm,
    LmStudio,
    OpenAiCompatible,
    Custom,
}

impl PersonalEngineChoice {
    pub const ALL: &'static [Self] = &[
        Self::Ollama,
        Self::Vllm,
        Self::LmStudio,
        Self::OpenAiCompatible,
        Self::Custom,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Ollama => crate::i18n::t(crate::i18n::keys::ENGINE_OLLAMA),
            Self::Vllm => crate::i18n::t(crate::i18n::keys::ENGINE_VLLM),
            Self::LmStudio => crate::i18n::t(crate::i18n::keys::ENGINE_LM_STUDIO),
            Self::OpenAiCompatible => crate::i18n::t(crate::i18n::keys::ENGINE_OPENAI_COMPAT),
            Self::Custom => crate::i18n::t(crate::i18n::keys::ENGINE_CUSTOM),
        }
    }

    #[must_use]
    pub const fn doc_url(self) -> &'static str {
        match self {
            Self::Ollama => "https://github.com/ollama/ollama/blob/main/docs/api.md",
            Self::Vllm => {
                "https://docs.vllm.ai/en/latest/serving/openai_compatible_server.html"
            }
            Self::LmStudio => "https://lmstudio.ai/docs/app/api/endpoints/openai",
            Self::OpenAiCompatible => "https://platform.openai.com/docs/api-reference",
            Self::Custom => "https://platform.openai.com/docs/api-reference",
        }
    }

    #[must_use]
    pub const fn preset_id(self) -> &'static str {
        use crate::engine::{
            PRESET_LM_STUDIO, PRESET_OLLAMA_LOCAL, PRESET_OPENAI_COMPAT, PRESET_VLLM_OPENAI,
        };
        match self {
            Self::Ollama => PRESET_OLLAMA_LOCAL,
            Self::Vllm => PRESET_VLLM_OPENAI,
            Self::LmStudio => PRESET_LM_STUDIO,
            Self::OpenAiCompatible => PRESET_OPENAI_COMPAT,
            Self::Custom => PRESET_OPENAI_COMPAT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudProviderChoice {
    OllamaCloud,
    Mistral,
    OvhCloud,
    HuggingFace,
    Scaleway,
}

impl CloudProviderChoice {
    pub const ALL: &'static [Self] = &[
        Self::OllamaCloud,
        Self::Mistral,
        Self::OvhCloud,
        Self::HuggingFace,
        Self::Scaleway,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::OllamaCloud => crate::i18n::t(crate::i18n::keys::CLOUD_OLLAMA),
            Self::Mistral => crate::i18n::t(crate::i18n::keys::CLOUD_MISTRAL),
            Self::OvhCloud => crate::i18n::t(crate::i18n::keys::CLOUD_OVH),
            Self::HuggingFace => crate::i18n::t(crate::i18n::keys::CLOUD_HF),
            Self::Scaleway => crate::i18n::t(crate::i18n::keys::CLOUD_SCALEWAY),
        }
    }

    #[must_use]
    pub const fn doc_url(self) -> &'static str {
        match self {
            Self::OllamaCloud => "https://ollama.com/cloud",
            Self::Mistral => "https://docs.mistral.ai/api/",
            Self::OvhCloud => {
                "https://help.ovhcloud.com/csm/en-gb-public-cloud-ai-machine-learning-endpoints"
            }
            Self::HuggingFace => "https://huggingface.co/docs/api-inference/index",
            Self::Scaleway => "https://www.scaleway.com/en/docs/generative-apis/",
        }
    }

    #[must_use]
    pub const fn preset_id(self) -> &'static str {
        use crate::engine::{
            PRESET_HF_CLOUD, PRESET_MISTRAL_CLOUD, PRESET_OLLAMA_CLOUD, PRESET_OVH_CLOUD,
            PRESET_SCALEWAY_CLOUD,
        };
        match self {
            Self::OllamaCloud => PRESET_OLLAMA_CLOUD,
            Self::Mistral => PRESET_MISTRAL_CLOUD,
            Self::OvhCloud => PRESET_OVH_CLOUD,
            Self::HuggingFace => PRESET_HF_CLOUD,
            Self::Scaleway => PRESET_SCALEWAY_CLOUD,
        }
    }

    #[must_use]
    pub const fn llm_provider(self) -> crate::engine::LlmProvider {
        use crate::engine::LlmProvider;
        match self {
            Self::OllamaCloud => LlmProvider::OllamaCloud,
            Self::Mistral => LlmProvider::MistralCloud,
            Self::OvhCloud => LlmProvider::OvhCloud,
            Self::HuggingFace => LlmProvider::HuggingFaceCloud,
            Self::Scaleway => LlmProvider::ScalewayCloud,
        }
    }

    #[must_use]
    pub const fn requires_bearer_auth(self) -> bool {
        self.llm_provider().requires_bearer_auth()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthTypeChoice {
    #[default]
    None,
    Bearer,
    ApiKeyHeader,
}

impl AuthTypeChoice {
    pub const ALL: &'static [Self] = &[Self::None, Self::Bearer, Self::ApiKeyHeader];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::None => crate::i18n::t(crate::i18n::keys::AUTH_NONE),
            Self::Bearer => crate::i18n::t(crate::i18n::keys::AUTH_BEARER),
            Self::ApiKeyHeader => crate::i18n::t(crate::i18n::keys::AUTH_API_KEY_HEADER),
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::None => Self::Bearer,
            Self::Bearer => Self::ApiKeyHeader,
            Self::ApiKeyHeader => Self::None,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::None => Self::ApiKeyHeader,
            Self::Bearer => Self::None,
            Self::ApiKeyHeader => Self::Bearer,
        }
    }
}

/// Champ actif — étape configuration connexion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigureField {
    Url,
    AuthType,
    AuthHeaderName,
    AuthToken,
    ExtraHeaderName,
    ExtraHeaderValue,
    AddExtraHeader,
    TestButton,
    BackButton,
}

/// Focus clavier sur l'étape choix du modèle (`/server`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiServerSelectFocus {
    #[default]
    ModelList,
    NumCtx,
    MaxIterations,
    ResetWizard,
}

/// Modal connexion serveur IA (`/server`, Ctrl+Shift+L).
#[derive(Debug, Clone)]
pub struct AiServerDialog {
    pub step: AiServerStep,
    pub list_cursor: usize,
    pub deployment: Option<DeploymentKind>,
    pub personal_engine: Option<PersonalEngineChoice>,
    pub cloud_provider: Option<CloudProviderChoice>,
    pub server: String,
    pub server_cursor: usize,
    pub auth_type: AuthTypeChoice,
    pub auth_header_name: String,
    pub auth_header_name_cursor: usize,
    pub auth_token: String,
    pub auth_token_cursor: usize,
    pub extra_header_name: String,
    pub extra_header_name_cursor: usize,
    pub extra_header_value: String,
    pub extra_header_value_cursor: usize,
    pub extra_headers: Vec<(String, String)>,
    pub configure_focus: ConfigureField,
    pub status: String,
    pub models: Vec<String>,
    pub model_cursor: usize,
    pub context_preset_index: usize,
    pub context_custom: String,
    pub context_custom_cursor: usize,
    pub max_iterations: String,
    pub max_iterations_cursor: usize,
    pub select_focus: AiServerSelectFocus,
}

impl AiServerDialog {
    #[must_use]
    pub fn new_wizard() -> Self {
        Self {
            step: AiServerStep::ChooseDeployment,
            list_cursor: 0,
            deployment: None,
            personal_engine: None,
            cloud_provider: None,
            server: String::new(),
            server_cursor: 0,
            auth_type: AuthTypeChoice::None,
            auth_header_name: "x-api-key".into(),
            auth_header_name_cursor: 9,
            auth_token: String::new(),
            auth_token_cursor: 0,
            extra_header_name: String::new(),
            extra_header_name_cursor: 0,
            extra_header_value: String::new(),
            extra_header_value_cursor: 0,
            extra_headers: Vec::new(),
            configure_focus: ConfigureField::Url,
            status: "Perso ou cloud ? · Entrée · Esc annuler".into(),
            models: Vec::new(),
            model_cursor: 0,
            context_preset_index: crate::engine::preset_index_for(crate::engine::default_num_ctx()),
            context_custom: String::new(),
            context_custom_cursor: 0,
            max_iterations: crate::engine::default_max_iterations().to_string(),
            max_iterations_cursor: 0,
            select_focus: AiServerSelectFocus::ModelList,
        }
    }

    #[must_use]
    pub fn from_saved(
        server: String,
        api_key: String,
        current_model: String,
        num_ctx: i64,
        max_iterations: usize,
        profile: Option<&crate::engine::ConnectionProfile>,
    ) -> Self {
        let mut dialog = Self::new_wizard();
        if let Some(p) = profile {
            dialog.hydrate_from_profile(p);
            if p.is_connection_verified() {
                dialog.models = p.verified_models.clone();
                let preferred = p
                    .default_model
                    .as_deref()
                    .filter(|m| !m.trim().is_empty())
                    .unwrap_or(current_model.as_str());
                dialog.model_cursor = dialog
                    .models
                    .iter()
                    .position(|m| m == preferred)
                    .unwrap_or(0);
                dialog.step = AiServerStep::SelectModel;
                dialog.select_focus = AiServerSelectFocus::ModelList;
                dialog.status = format!(
                    "Connexion enregistree ({}) — choisissez le modele",
                    p.name
                );
            }
        } else {
            dialog.server = server.clone();
            dialog.server_cursor = server.len();
            if !api_key.is_empty() {
                use crate::engine::infer_provider_from_url;
                let provider = infer_provider_from_url(&server);
                if provider.is_cloud() || provider.uses_openai_api() {
                    dialog.auth_type = AuthTypeChoice::Bearer;
                } else {
                    dialog.auth_type = AuthTypeChoice::ApiKeyHeader;
                }
                dialog.auth_token = api_key.clone();
                dialog.auth_token_cursor = api_key.len();
            }
            dialog.context_preset_index = crate::engine::preset_index_for(num_ctx);
            if dialog.context_preset_index == crate::engine::CONTEXT_CUSTOM_INDEX {
                dialog.context_custom = num_ctx.to_string();
                dialog.context_custom_cursor = dialog.context_custom.len();
            }
            dialog.max_iterations = max_iterations.to_string();
            dialog.max_iterations_cursor = dialog.max_iterations.len();
        }
        dialog
    }

    pub fn begin_reset_wizard(&mut self) {
        self.step = AiServerStep::ConfirmReset;
        self.status =
            "La connexion actuelle reste active tant que la nouvelle n'est pas validee.".into();
    }

    pub fn hydrate_from_profile(&mut self, profile: &crate::engine::ConnectionProfile) {
        use crate::engine::LlmProvider;
        match profile.provider {
            LlmProvider::OllamaCloud => {
                self.deployment = Some(DeploymentKind::Cloud);
                self.cloud_provider = Some(CloudProviderChoice::OllamaCloud);
                self.personal_engine = None;
            }
            LlmProvider::MistralCloud => {
                self.deployment = Some(DeploymentKind::Cloud);
                self.cloud_provider = Some(CloudProviderChoice::Mistral);
                self.personal_engine = None;
            }
            LlmProvider::OvhCloud => {
                self.deployment = Some(DeploymentKind::Cloud);
                self.cloud_provider = Some(CloudProviderChoice::OvhCloud);
                self.personal_engine = None;
            }
            LlmProvider::HuggingFaceCloud => {
                self.deployment = Some(DeploymentKind::Cloud);
                self.cloud_provider = Some(CloudProviderChoice::HuggingFace);
                self.personal_engine = None;
            }
            LlmProvider::ScalewayCloud => {
                self.deployment = Some(DeploymentKind::Cloud);
                self.cloud_provider = Some(CloudProviderChoice::Scaleway);
                self.personal_engine = None;
            }
            LlmProvider::OllamaLocal => {
                self.deployment = Some(DeploymentKind::Personal);
                self.personal_engine = Some(PersonalEngineChoice::Ollama);
            }
            LlmProvider::Vllm => {
                self.deployment = Some(DeploymentKind::Personal);
                self.personal_engine = Some(PersonalEngineChoice::Vllm);
            }
            LlmProvider::LmStudio => {
                self.deployment = Some(DeploymentKind::Personal);
                self.personal_engine = Some(PersonalEngineChoice::LmStudio);
            }
            LlmProvider::OpenAiCompatible => {
                self.deployment = Some(DeploymentKind::Personal);
                self.personal_engine = Some(PersonalEngineChoice::OpenAiCompatible);
            }
            LlmProvider::Custom => {
                self.deployment = Some(DeploymentKind::Personal);
                self.personal_engine = Some(PersonalEngineChoice::Custom);
            }
        }
        self.server = profile.base_url.clone();
        self.server_cursor = self.server.len();
        self.auth_type = match &profile.auth {
            crate::engine::AuthConfig::Bearer { token } => {
                self.auth_token = token.clone();
                self.auth_token_cursor = token.len();
                AuthTypeChoice::Bearer
            }
            crate::engine::AuthConfig::ApiKeyHeader { header_name, token } => {
                self.auth_header_name = header_name.clone();
                self.auth_header_name_cursor = header_name.len();
                self.auth_token = token.clone();
                self.auth_token_cursor = token.len();
                AuthTypeChoice::ApiKeyHeader
            }
            crate::engine::AuthConfig::CustomHeaders => AuthTypeChoice::None,
            crate::engine::AuthConfig::None => AuthTypeChoice::None,
        };
        self.extra_headers = profile
            .extra_headers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        self.context_preset_index = crate::engine::preset_index_for(profile.num_ctx);
        if self.context_preset_index == crate::engine::CONTEXT_CUSTOM_INDEX {
            self.context_custom = profile.num_ctx.to_string();
            self.context_custom_cursor = self.context_custom.len();
        }
        self.max_iterations = profile.max_iterations.to_string();
        self.max_iterations_cursor = self.max_iterations.len();
    }

    pub fn apply_engine_defaults(&mut self) {
        use crate::engine::{builtin_preset_templates, AuthConfig, LlmProvider};
        let preset_id = match self.deployment {
            Some(DeploymentKind::Cloud) => self.cloud_provider.map(CloudProviderChoice::preset_id),
            Some(DeploymentKind::Personal) => {
                self.personal_engine.map(PersonalEngineChoice::preset_id)
            }
            None => None,
        };
        let Some(preset_id) = preset_id else {
            return;
        };
        if self.personal_engine == Some(PersonalEngineChoice::Custom) {
            self.server.clear();
            self.server_cursor = 0;
            self.auth_type = AuthTypeChoice::Bearer;
            self.auth_token.clear();
            self.auth_token_cursor = 0;
            self.auth_header_name = "x-api-key".into();
            self.auth_header_name_cursor = 9;
            return;
        }
        let Some(template) = builtin_preset_templates()
            .into_iter()
            .find(|p| p.id == preset_id)
        else {
            return;
        };
        self.server = template.base_url;
        self.server_cursor = self.server.len();
        match template.auth {
            AuthConfig::None => {
                self.auth_type = AuthTypeChoice::None;
                self.auth_token.clear();
                self.auth_token_cursor = 0;
            }
            AuthConfig::Bearer { token } => {
                self.auth_type = AuthTypeChoice::Bearer;
                self.auth_token = token;
                self.auth_token_cursor = self.auth_token.len();
            }
            AuthConfig::ApiKeyHeader { header_name, token } => {
                self.auth_type = AuthTypeChoice::ApiKeyHeader;
                self.auth_header_name = header_name;
                self.auth_header_name_cursor = self.auth_header_name.len();
                self.auth_token = token;
                self.auth_token_cursor = self.auth_token.len();
            }
            AuthConfig::CustomHeaders => {
                self.auth_type = AuthTypeChoice::None;
            }
        }
        if template.provider == LlmProvider::Custom {
            self.auth_header_name = "x-api-key".into();
            self.auth_header_name_cursor = 9;
        }
        let _ = template;
    }

    pub fn wizard_back(&mut self) {
        self.status.clear();
        self.step = match self.step {
            AiServerStep::ConfirmReset => AiServerStep::SelectModel,
            AiServerStep::SelectModel => AiServerStep::ConfigureConnection,
            AiServerStep::ConfigureConnection => match self.deployment {
                Some(DeploymentKind::Personal) => AiServerStep::ChoosePersonalEngine,
                Some(DeploymentKind::Cloud) => AiServerStep::ChooseCloudProvider,
                None => AiServerStep::ChooseDeployment,
            },
            AiServerStep::ChoosePersonalEngine | AiServerStep::ChooseCloudProvider => {
                AiServerStep::ChooseDeployment
            }
            AiServerStep::ChooseDeployment | AiServerStep::Testing => AiServerStep::ChooseDeployment,
        };
        self.set_step_hint();
    }

    pub fn wizard_advance_list(&mut self) {
        match self.step {
            AiServerStep::ChooseDeployment => {
                self.deployment = DeploymentKind::ALL.get(self.list_cursor).copied();
                self.step = match self.deployment {
                    Some(DeploymentKind::Personal) => AiServerStep::ChoosePersonalEngine,
                    Some(DeploymentKind::Cloud) => AiServerStep::ChooseCloudProvider,
                    None => AiServerStep::ChooseDeployment,
                };
                self.list_cursor = 0;
            }
            AiServerStep::ChoosePersonalEngine => {
                self.personal_engine = PersonalEngineChoice::ALL.get(self.list_cursor).copied();
                self.apply_engine_defaults();
                self.step = AiServerStep::ConfigureConnection;
                self.configure_focus = ConfigureField::Url;
            }
            AiServerStep::ChooseCloudProvider => {
                self.cloud_provider = CloudProviderChoice::ALL.get(self.list_cursor).copied();
                self.apply_engine_defaults();
                self.step = AiServerStep::ConfigureConnection;
                self.configure_focus = ConfigureField::Url;
            }
            _ => {}
        }
        self.set_step_hint();
    }

    fn set_step_hint(&mut self) {
        self.status = match self.step {
            AiServerStep::ChooseDeployment => {
                "Etape 1/3 · Perso ou cloud ? · fleches · Entree · Esc annuler".into()
            }
            AiServerStep::ChoosePersonalEngine => {
                "Etape 2/3 · Moteur d'inference · fleches · Entree · Esc retour".into()
            }
            AiServerStep::ChooseCloudProvider => {
                "Etape 2/3 · Prestataire cloud · fleches · Entree · Esc retour".into()
            }
            AiServerStep::ConfigureConnection => {
                "Etape 3/3 · Connexion · Tab · Tester · Esc retour".into()
            }
            AiServerStep::Testing => "Test de connexion en cours…".into(),
            AiServerStep::SelectModel => {
                "Modele et contexte · Tab · Entree appliquer · Esc retour connexion".into()
            }
            AiServerStep::ConfirmReset => {
                "Reinitialiser l'assistant connexion · Entree confirmer · Esc annuler".into()
            }
        };
    }

    pub fn add_extra_header_from_inputs(&mut self) {
        let name = self.extra_header_name.trim().to_string();
        let value = self.extra_header_value.trim().to_string();
        if name.is_empty() || value.is_empty() {
            self.status = "Nom et valeur header requis".into();
            return;
        }
        self.extra_headers
            .retain(|(k, _)| !k.eq_ignore_ascii_case(&name));
        self.extra_headers.push((name.clone(), value));
        self.extra_header_name.clear();
        self.extra_header_name_cursor = 0;
        self.extra_header_value.clear();
        self.extra_header_value_cursor = 0;
        self.status = format!("Header `{name}` ajoute");
    }

    #[must_use]
    pub fn build_probe_profile(&self) -> crate::engine::ConnectionProfile {
        use crate::engine::{AuthConfig, ConnectionProfile, LlmProvider};
        let provider = match self.deployment {
            Some(DeploymentKind::Cloud) => self
                .cloud_provider
                .map(CloudProviderChoice::llm_provider)
                .unwrap_or(crate::engine::LlmProvider::OllamaCloud),
            Some(DeploymentKind::Personal) => match self.personal_engine {
                Some(PersonalEngineChoice::Ollama) => LlmProvider::OllamaLocal,
                Some(PersonalEngineChoice::Vllm) => LlmProvider::Vllm,
                Some(PersonalEngineChoice::LmStudio) => LlmProvider::LmStudio,
                Some(PersonalEngineChoice::OpenAiCompatible) => LlmProvider::OpenAiCompatible,
                Some(PersonalEngineChoice::Custom) | None => LlmProvider::Custom,
            },
            None => LlmProvider::Custom,
        };
        let auth = match self.auth_type {
            AuthTypeChoice::None => AuthConfig::None,
            AuthTypeChoice::Bearer => AuthConfig::Bearer {
                token: self.auth_token.clone(),
            },
            AuthTypeChoice::ApiKeyHeader => AuthConfig::ApiKeyHeader {
                header_name: self.auth_header_name.clone(),
                token: self.auth_token.clone(),
            },
        };
        let num_ctx = self.resolved_num_ctx().unwrap_or(crate::engine::default_num_ctx());
        let max_iterations = self
            .resolved_max_iterations()
            .unwrap_or(crate::engine::default_max_iterations());
        let name = match self.deployment {
            Some(DeploymentKind::Cloud) => self
                .cloud_provider
                .map(CloudProviderChoice::label)
                .unwrap_or("Cloud")
                .to_string(),
            Some(DeploymentKind::Personal) => self
                .personal_engine
                .map(PersonalEngineChoice::label)
                .unwrap_or("Perso")
                .to_string(),
            None => "Connexion".into(),
        };
        ConnectionProfile {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            provider,
            base_url: self.server.trim().to_string(),
            default_model: self.models.get(self.model_cursor).cloned(),
            auth,
            extra_headers: self
                .extra_headers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            num_ctx,
            max_iterations,
            built_in: false,
            preset_origin: None,
            connection_verified: false,
            verified_models: Vec::new(),
        }
    }

    pub fn configure_active_buffer(&mut self) -> Option<(&mut String, &mut usize)> {
        match self.configure_focus {
            ConfigureField::Url => Some((&mut self.server, &mut self.server_cursor)),
            ConfigureField::AuthHeaderName => {
                Some((&mut self.auth_header_name, &mut self.auth_header_name_cursor))
            }
            ConfigureField::AuthToken => Some((&mut self.auth_token, &mut self.auth_token_cursor)),
            ConfigureField::ExtraHeaderName => {
                Some((&mut self.extra_header_name, &mut self.extra_header_name_cursor))
            }
            ConfigureField::ExtraHeaderValue => Some((
                &mut self.extra_header_value,
                &mut self.extra_header_value_cursor,
            )),
            ConfigureField::AuthType
            | ConfigureField::AddExtraHeader
            | ConfigureField::TestButton
            | ConfigureField::BackButton => None,
        }
    }

    pub fn configure_doc_url(&self) -> Option<&'static str> {
        match self.deployment {
            Some(DeploymentKind::Cloud) => self.cloud_provider.map(CloudProviderChoice::doc_url),
            Some(DeploymentKind::Personal) => self.personal_engine.map(PersonalEngineChoice::doc_url),
            None => None,
        }
    }

    #[must_use]
    pub fn auth_type_locked(&self) -> bool {
        match self.deployment {
            Some(DeploymentKind::Cloud) => self
                .cloud_provider
                .is_some_and(CloudProviderChoice::requires_bearer_auth),
            _ => false,
        }
    }

    pub fn cycle_configure_focus(&mut self, reverse: bool) {
        let order = [
            ConfigureField::Url,
            ConfigureField::AuthType,
            ConfigureField::AuthHeaderName,
            ConfigureField::AuthToken,
            ConfigureField::ExtraHeaderName,
            ConfigureField::ExtraHeaderValue,
            ConfigureField::AddExtraHeader,
            ConfigureField::TestButton,
            ConfigureField::BackButton,
        ];
        let skip_header = self.auth_type != AuthTypeChoice::ApiKeyHeader;
        let skip_auth_type = self.auth_type_locked();
        let pos = order
            .iter()
            .position(|&f| f == self.configure_focus)
            .unwrap_or(0);
        let mut next = pos;
        loop {
            next = if reverse {
                if next == 0 {
                    order.len() - 1
                } else {
                    next - 1
                }
            } else if next + 1 >= order.len() {
                0
            } else {
                next + 1
            };
            let candidate = order[next];
            if skip_header && candidate == ConfigureField::AuthHeaderName {
                continue;
            }
            if skip_auth_type && candidate == ConfigureField::AuthType {
                continue;
            }
            self.configure_focus = candidate;
            break;
        }
    }

    pub fn resolved_num_ctx(&self) -> Result<i64, String> {
        crate::engine::resolve_num_ctx(self.context_preset_index, &self.context_custom)
    }

    pub fn resolved_max_iterations(&self) -> Result<usize, String> {
        crate::engine::parse_max_iterations(&self.max_iterations)
    }
}

/// Étape modal `/workspace`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceStep {
    Edit,
    Confirm,
}

/// Focus clavier dans le modal workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceField {
    Browser,
    SelectButton,
    Path,
}

/// Modal changement workspace (`/workspace`, Ctrl+Shift+W).
#[derive(Debug, Clone)]
pub struct WorkspaceDialog {
    pub location: crate::engine::dir_browser::BrowseLocation,
    pub entries: Vec<crate::engine::dir_browser::BrowseEntry>,
    pub browse_cursor: usize,
    pub path: String,
    pub path_cursor: usize,
    pub focus: WorkspaceField,
    pub step: WorkspaceStep,
    pub status: String,
    pub validated: Option<Utf8PathBuf>,
}

impl WorkspaceDialog {
    #[must_use]
    pub fn new(current_path: String, _recents: Vec<String>, initial: Option<String>) -> Self {
        use crate::engine::dir_browser::{list_location, BrowseEntryKind, BrowseLocation};

        let hint = initial.unwrap_or(current_path);
        let location = BrowseLocation::from_hint(&hint);
        let entries = list_location(&location);
        let browse_cursor = entries
            .iter()
            .position(|e| e.kind != BrowseEntryKind::Parent)
            .unwrap_or(0);
        let path = location.display_path();
        Self {
            location,
            entries,
            browse_cursor,
            path: path.clone(),
            path_cursor: path.len(),
            focus: WorkspaceField::Browser,
            step: WorkspaceStep::Edit,
            status: "↑↓ naviguer · Entrée ouvrir · Tab · Choisir ce dossier".into(),
            validated: None,
        }
    }

    pub fn reload_browser(&mut self) {
        use crate::engine::dir_browser::list_location;

        self.entries = list_location(&self.location);
        if self.browse_cursor >= self.entries.len() {
            self.browse_cursor = self.entries.len().saturating_sub(1);
        }
        if self.focus != WorkspaceField::Path {
            self.path = self.location.display_path();
            self.path_cursor = self.path.len();
        }
    }
}

/// Entrée affichable dans le sélecteur rewind.
#[derive(Debug, Clone)]
pub struct RewindChoiceView {
    pub message_index: usize,
    pub label: String,
    pub restore_text: String,
}

/// Sélecteur `/theme`.
#[derive(Debug, Clone)]
pub struct ThemeDialog {
    pub cursor: usize,
}

/// Ligne éditable dans la modale `/settings` (extensible).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRowKind {
    Language,
    Animations,
    Mouse,
    Vim,
    Updates,
}

impl SettingsRowKind {
    pub const ALL: &'static [Self] = &[
        Self::Language,
        Self::Animations,
        Self::Mouse,
        Self::Vim,
        Self::Updates,
    ];
}

/// Modale réglages TUI (`/settings`).
#[derive(Debug, Clone)]
pub struct SettingsDialog {
    pub cursor: usize,
}

/// Confirmation installation MAJ (`/update install`).
#[derive(Debug, Clone)]
pub struct UpdateInstallDialog {
    pub from_version: String,
    pub to_version: String,
    pub sha256: String,
    pub release_notes: Option<String>,
}

/// Sélecteur `/copy` — réponse complète ou bloc de code.
#[derive(Debug, Clone)]
pub struct CopyDialog {
    pub full_text: String,
    pub blocks: Vec<crate::engine::copy_cmd::CopyCodeBlock>,
    pub choices: Vec<crate::engine::copy_cmd::CopyChoice>,
    pub cursor: usize,
}

impl CopyDialog {
    #[must_use]
    pub fn new(
        full_text: String,
        blocks: Vec<crate::engine::copy_cmd::CopyCodeBlock>,
    ) -> Self {
        use crate::engine::copy_cmd::CopyChoice;
        let mut choices = vec![CopyChoice::FullResponse];
        for i in 0..blocks.len() {
            choices.push(CopyChoice::CodeBlock(i));
        }
        choices.push(CopyChoice::AlwaysFullResponse);
        Self {
            full_text,
            blocks,
            choices,
            cursor: 0,
        }
    }
}

/// Popup suggestions composer (`@` · `/` · skills).
pub use crate::engine::unified_suggestions::ComposerSuggestionDialog;

/// Sélecteur palette slash (`/`).
#[derive(Debug, Clone)]
pub struct SlashPaletteDialog {
    pub filter: String,
    pub matches: Vec<usize>,
    pub cursor: usize,
}

/// État UI persistant pendant la session TUI.
pub struct AppState {
    pub phase: AppPhase,
    pub theme: TuiThemeSetting,
    pub session_accent: Option<SessionAccent>,
    pub palette: ThemePalette,
    pub composer_mode: ComposerMode,
    pub composer_buffer: String,
    pub status_line: String,
    pub last_run: RunStatus,
    pub entries: Vec<LogEntry>,
    pub streaming: Option<String>,
    pub scroll: u16,
    pub prompt: Option<PromptDialog>,
    pub rewind: Option<RewindDialog>,
    pub theme_dialog: Option<ThemeDialog>,
    pub settings_dialog: Option<SettingsDialog>,
    /// Confirmation `/update install`.
    pub update_install: Option<UpdateInstallDialog>,
    pub pending_ask: bool,
    /// Messages en attente pendant un run agent.
    pub queued_messages: usize,
    /// Sorties outils développées dans le fil (`e` : bash, file_read, grep, glob, web_fetch, web_search).
    pub expanded_tools: HashSet<ToolUseId>,
    /// Blocs phase repliés (index `PhaseOpen` dans `entries`).
    pub collapsed_phases: HashSet<usize>,
    /// Dernier état `todo_write` (panneau live).
    pub todo_snapshot: Option<TodoSnapshot>,
    /// Dernier plan de cours (`course_plan_write`).
    pub course_snapshot: Option<CourseSnapshot>,
    /// Serveurs MCP configurés (panneau live).
    pub mcp_snapshot: Option<crate::engine::McpPanelSnapshot>,
    /// Onboarding premier lancement.
    pub onboarding: Option<OnboardingDialog>,
    /// Connexion serveur IA (`/server`).
    pub ai_server: Option<AiServerDialog>,
    /// Changement workspace (`/workspace`).
    pub workspace_dialog: Option<WorkspaceDialog>,
    /// Toast éphémère (coin bas-droit).
    pub toast: Option<crate::widgets::toast::Toast>,
    /// Recherche dans le fil (`Ctrl+F`).
    pub transcript_search: Option<TranscriptSearchState>,
    pub slash_palette: Option<SlashPaletteDialog>,
    /// Sélecteur `/copy`.
    pub copy: Option<CopyDialog>,
    /// Complétion unifiée (`@` · `/` · skills).
    pub composer_suggestions: Option<ComposerSuggestionDialog>,
    /// Menu d'aide composer (`?`).
    pub composer_help: bool,
    /// Bannières contextuelles sous le header (§11.4 / §11.5).
    pub status_notices: Vec<crate::engine::notices::StatusNotice>,
    /// Bash en cours (`ToolStart` sans `ToolFinish` encore).
    pub active_bash: Option<ActiveBashRun>,
    /// Hook Pre/Post tool en cours.
    pub hook_progress: Option<ActiveHookProgress>,
    /// Viewer scrollable outil (`e` : bash, file_read, grep, glob, web_fetch, web_search, diff, lsp, skill, mcp, task).
    pub scroll_viewer: Option<ScrollViewerState>,
    /// Compteur frame pour spinner bash.
    pub ui_frame_tick: u8,
    /// Animation entrée modale (0–4).
    pub modal_anim_tick: u8,
    /// Animations UI (cursor-blink, pulse, modal-in).
    pub animations_enabled: bool,
    /// Capture et traitement souris (scroll, clic modales).
    pub mouse_enabled: bool,
    /// Ligne survolée dans le fil (index visible, 0 = haut).
    pub hover_log_row: Option<usize>,
    /// Rectangles hit-test mis à jour chaque frame.
    pub hit_areas: crate::ui::hit_areas::HitAreas,
    /// Permissions/questions encore en file (hors modal courante).
    pub permission_queue_waiting: usize,
    /// Titre affichable (`ses_*.meta.json` ou id session).
    pub session_title: String,
    /// Début du run agent courant (spinner §3.7).
    pub run_started: Option<std::time::Instant>,
    /// Phase agent active (`PhaseEnter` sans `PhaseClose`).
    pub active_phase: Option<drox_engine::Phase>,
    /// Compteur d'invalidation du cache de rendu fil.
    pub log_revision: u64,
    /// Connexion IA validée (`/server` ou CLI explicite).
    pub llm_configured: bool,
    /// Version distante disponible (bandeau + pastille header).
    pub update_available_version: Option<String>,
    /// Cache lignes fil (hors streaming / bash live).
    log_render_cache: Option<crate::view::log_cache::LogRenderCache>,
    /// Diffs fichiers du dernier run terminé (bandeau Entrée / `e`).
    pub run_diff_summary: Option<crate::view::tool_output::RunDiffSummary>,
}

/// Cible du dernier outil expansible dans le fil.
enum LatestExpandable<'a> {
    FileRead {
        id: ToolUseId,
        output: &'a Value,
    },
    Bash {
        id: ToolUseId,
        output: &'a Value,
        is_error: bool,
        user_mode: bool,
        command: Option<String>,
    },
    Grep {
        id: ToolUseId,
        output: &'a Value,
    },
    Glob {
        id: ToolUseId,
        output: &'a Value,
        pattern: Option<String>,
    },
    WebFetch {
        id: ToolUseId,
        output: &'a Value,
    },
    WebSearch {
        id: ToolUseId,
        output: &'a Value,
    },
    FileEdit {
        id: ToolUseId,
        output: &'a Value,
    },
    FileWrite {
        id: ToolUseId,
        output: &'a Value,
    },
    NotebookEdit {
        id: ToolUseId,
        output: &'a Value,
    },
    Lsp {
        id: ToolUseId,
        output: &'a Value,
    },
    SkillRead {
        id: ToolUseId,
        output: &'a Value,
    },
    Task {
        id: ToolUseId,
        output: &'a Value,
    },
    Mcp {
        id: ToolUseId,
        name: String,
        output: &'a Value,
    },
    ExitPlanMode {
        id: ToolUseId,
        plan: String,
    },
    SkillList {
        id: ToolUseId,
        output: &'a Value,
    },
    Worktree {
        id: ToolUseId,
        name: String,
        output: &'a Value,
    },
    PlanDocument {
        path: String,
        body: String,
    },
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        let prefs = crate::engine::preferences::load_preferences();
        let palette = resolve_palette(prefs.theme, prefs.session_color);
        Self {
            theme: prefs.theme,
            session_accent: prefs.session_color,
            palette,
            status_line: crate::i18n::t(crate::i18n::keys_p1::STATUS_READY).into(),
            entries: Vec::new(),
            streaming: None,
            scroll: 0,
            prompt: None,
            rewind: None,
            theme_dialog: None,
            settings_dialog: None,
            update_install: None,
            pending_ask: false,
            queued_messages: 0,
            expanded_tools: HashSet::new(),
            collapsed_phases: HashSet::new(),
            todo_snapshot: None,
            course_snapshot: None,
            mcp_snapshot: None,
            onboarding: None,
            ai_server: None,
            workspace_dialog: None,
            toast: None,
            transcript_search: None,
            slash_palette: None,
            copy: None,
            composer_suggestions: None,
            composer_help: false,
            status_notices: Vec::new(),
            active_bash: None,
            hook_progress: None,
            scroll_viewer: None,
            ui_frame_tick: 0,
            modal_anim_tick: 0,
            animations_enabled: prefs.animations_enabled,
            mouse_enabled: prefs.mouse_enabled,
            hover_log_row: None,
            hit_areas: crate::ui::hit_areas::HitAreas::default(),
            permission_queue_waiting: 0,
            session_title: String::new(),
            run_started: None,
            active_phase: None,
            log_revision: 0,
            log_render_cache: None,
            run_diff_summary: None,
            phase: AppPhase::Idle,
            composer_mode: ComposerMode::Normal,
            composer_buffer: String::new(),
            last_run: RunStatus::None,
            llm_configured: false,
            update_available_version: None,
        }
    }

    pub fn reset_modal_anim(&mut self) {
        self.modal_anim_tick = 0;
    }

    pub fn tick_modal_anim(&mut self) {
        if self.phase.is_modal() {
            if self.modal_anim_tick < 4 {
                self.modal_anim_tick = self.modal_anim_tick.saturating_add(1);
            }
        } else {
            self.modal_anim_tick = 0;
        }
    }

    pub fn push_entry(&mut self, entry: LogEntry) {
        self.entries.push(entry);
        const MAX: usize = 500;
        if self.entries.len() > MAX {
            self.entries.drain(0..self.entries.len() - MAX);
        }
        self.bump_log_revision();
    }

    fn bump_log_revision(&mut self) {
        self.log_revision = self.log_revision.wrapping_add(1);
        self.log_render_cache = None;
    }

    pub fn push_system(&mut self, text: impl Into<String>) {
        self.push_entry(LogEntry::System { text: text.into() });
    }

    pub fn push_user(&mut self, text: impl Into<String>) {
        self.push_entry(LogEntry::User { text: text.into() });
    }

    pub fn clear_transcript(&mut self) {
        self.entries.clear();
        self.streaming = None;
        self.scroll = 0;
        self.expanded_tools.clear();
        self.collapsed_phases.clear();
        self.todo_snapshot = None;
        self.course_snapshot = None;
        self.status_notices.clear();
        self.active_bash = None;
        self.hook_progress = None;
        self.scroll_viewer = None;
        self.run_started = None;
        self.active_phase = None;
        self.bump_log_revision();
    }

    /// Met à jour le panneau todos depuis un output `todo_write`.
    pub fn apply_todo_output(&mut self, output: &Value) {
        let Some(items) = parse_todo_items(output) else {
            return;
        };
        let summary = output
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        self.todo_snapshot = Some(TodoSnapshot { items, summary });
    }

    /// Met à jour le panneau cours depuis un output `course_plan_write`.
    pub fn apply_course_plan_output(&mut self, output: &Value) {
        let Some((title, steps, summary)) = parse_course_plan(output) else {
            return;
        };
        self.course_snapshot = Some(CourseSnapshot {
            title,
            steps,
            summary,
        });
    }

    /// Reconstruit todos + plan cours depuis le fil (rejeu transcript).
    pub fn rebuild_side_panels_from_entries(&mut self) {
        self.todo_snapshot = None;
        self.course_snapshot = None;
        let todo_outputs: Vec<(String, Value)> = self
            .entries
            .iter()
            .filter_map(|entry| {
                if let LogEntry::ToolFinish {
                    name,
                    output,
                    is_error: false,
                    ..
                } = entry
                {
                    Some((name.clone(), output.clone()))
                } else {
                    None
                }
            })
            .collect();
        for (name, output) in todo_outputs {
            match name.as_str() {
                "todo_write" => self.apply_todo_output(&output),
                "course_plan_write" => self.apply_course_plan_output(&output),
                _ => {}
            }
        }
    }

    /// Bascule l'affichage développé du dernier outil expansible ou bloc phase.
    pub fn toggle_latest_expandable(&mut self) -> Option<&'static str> {
        match self.latest_expandable() {
            Some(LatestExpandable::FileRead { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::file_read_viewer::FileReadViewerState::from_output(
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::FileRead(viewer))
                    .then_some("file_read")
            }
            Some(LatestExpandable::Bash {
                id,
                output,
                is_error,
                user_mode,
                command,
            }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::bash_viewer::BashViewerState::from_output(
                    id.clone(),
                    &output,
                    is_error,
                    user_mode,
                    command,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Bash(viewer))
                    .then_some("bash")
            }
            Some(LatestExpandable::Grep { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer =
                    crate::view::grep_viewer::GrepViewerState::from_output(id.clone(), &output)?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Grep(viewer))
                    .then_some("grep")
            }
            Some(LatestExpandable::Glob { id, output, pattern }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::glob_viewer::GlobViewerState::from_output(
                    id.clone(),
                    &output,
                    pattern,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Glob(viewer))
                    .then_some("glob")
            }
            Some(LatestExpandable::WebFetch { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::web_fetch_viewer::WebFetchViewerState::from_output(
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::WebFetch(viewer))
                    .then_some("web_fetch")
            }
            Some(LatestExpandable::WebSearch { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::web_search_viewer::WebSearchViewerState::from_output(
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::WebSearch(viewer))
                    .then_some("web_search")
            }
            Some(LatestExpandable::FileEdit { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::tool_output::diff_viewer_from_output(
                    "file_edit",
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("file_edit")
            }
            Some(LatestExpandable::FileWrite { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::tool_output::diff_viewer_from_output(
                    "file_write",
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("file_write")
            }
            Some(LatestExpandable::NotebookEdit { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::tool_output::diff_viewer_from_output(
                    "notebook_edit",
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("notebook_edit")
            }
            Some(LatestExpandable::Lsp { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer =
                    crate::view::extra_output::lsp_viewer_from_output(id.clone(), &output)?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("lsp")
            }
            Some(LatestExpandable::SkillRead { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::extra_output::skill_read_viewer_from_output(
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("skill_read")
            }
            Some(LatestExpandable::Task { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer =
                    crate::view::extra_output::task_viewer_from_output(id.clone(), &output)?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("task")
            }
            Some(LatestExpandable::Mcp { id, name, output }) => {
                let id = id.clone();
                let name = name.clone();
                let output = output.clone();
                let viewer = crate::view::mcp_output::mcp_viewer_from_output(
                    &name,
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("mcp")
            }
            Some(LatestExpandable::ExitPlanMode { id, plan }) => {
                let id = id.clone();
                let viewer = crate::view::extra_output::exit_plan_mode_viewer_from_plan(
                    id.clone(),
                    &plan,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("exit_plan_mode")
            }
            Some(LatestExpandable::SkillList { id, output }) => {
                let id = id.clone();
                let output = output.clone();
                let viewer = crate::view::extra_output::skill_list_viewer_from_output(
                    id.clone(),
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("skill_list")
            }
            Some(LatestExpandable::Worktree { id, name, output }) => {
                let id = id.clone();
                let name = name.clone();
                let output = output.clone();
                let viewer = crate::view::extra_output::worktree_viewer_from_output(
                    id.clone(),
                    &name,
                    &output,
                )?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("worktree")
            }
            Some(LatestExpandable::PlanDocument { path, body }) => {
                let id = ToolUseId::from_string(format!("plan-doc:{path}"));
                let viewer =
                    crate::view::extra_output::plan_document_viewer(id.clone(), &path, &body)?;
                self.toggle_scroll_viewer(id, ScrollViewerState::Lines(viewer))
                    .then_some("plan")
            }
            None => self.toggle_latest_phase_block().then_some("phase"),
        }
    }

    fn latest_expandable(&self) -> Option<LatestExpandable<'_>> {
        for entry in self.entries.iter().rev() {
            match entry {
                LogEntry::PlanDocument {
                    path,
                    body,
                    truncated,
                } if crate::view::extra_output::plan_document_expandable(body, *truncated) => {
                    return Some(LatestExpandable::PlanDocument {
                        path: path.clone(),
                        body: body.clone(),
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "file_read" => {
                    return Some(LatestExpandable::FileRead {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error,
                } if name == "bash" => {
                    return Some(LatestExpandable::Bash {
                        id: id.clone(),
                        output,
                        is_error: *is_error,
                        user_mode: false,
                        command: output
                            .get("command")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "grep" => {
                    return Some(LatestExpandable::Grep {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "glob" => {
                    let pattern = self.glob_pattern_for(id);
                    return Some(LatestExpandable::Glob {
                        id: id.clone(),
                        output,
                        pattern,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "web_fetch" => {
                    return Some(LatestExpandable::WebFetch {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "web_search" => {
                    return Some(LatestExpandable::WebSearch {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "file_edit"
                    && crate::view::tool_output::diff_is_expandable(output) =>
                {
                    return Some(LatestExpandable::FileEdit {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "file_write"
                    && crate::view::tool_output::diff_is_expandable(output) =>
                {
                    return Some(LatestExpandable::FileWrite {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "notebook_edit"
                    && crate::view::tool_output::diff_is_expandable(output) =>
                {
                    return Some(LatestExpandable::NotebookEdit {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "lsp" && crate::view::extra_output::lsp_is_expandable(output) => {
                    return Some(LatestExpandable::Lsp {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "skill_read"
                    && crate::view::extra_output::skill_read_is_expandable(output) =>
                {
                    return Some(LatestExpandable::SkillRead {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "task" && crate::view::extra_output::task_is_expandable(output) => {
                    return Some(LatestExpandable::Task {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if crate::view::mcp_output::is_mcp_tool(name)
                    && crate::view::mcp_output::mcp_is_expandable(name, output) =>
                {
                    return Some(LatestExpandable::Mcp {
                        id: id.clone(),
                        name: name.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "exit_plan_mode" => {
                    if let Some(plan) = self.exit_plan_mode_plan_for(id) {
                        if crate::view::extra_output::exit_plan_mode_plan_expandable(&plan) {
                            return Some(LatestExpandable::ExitPlanMode {
                                id: id.clone(),
                                plan,
                            });
                        }
                    }
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if name == "skill_list"
                    && crate::view::extra_output::skill_list_is_expandable(output) =>
                {
                    return Some(LatestExpandable::SkillList {
                        id: id.clone(),
                        output,
                    });
                }
                LogEntry::ToolFinish {
                    id,
                    name,
                    output,
                    is_error: false,
                } if (name == "git_worktree_enter" || name == "git_worktree_exit")
                    && crate::view::extra_output::worktree_is_expandable(output) =>
                {
                    return Some(LatestExpandable::Worktree {
                        id: id.clone(),
                        name: name.clone(),
                        output,
                    });
                }
                LogEntry::BashModeOutput {
                    id,
                    output,
                    is_error,
                } => {
                    let command = self.bash_mode_command_for(id);
                    return Some(LatestExpandable::Bash {
                        id: id.clone(),
                        output,
                        is_error: *is_error,
                        user_mode: true,
                        command,
                    });
                }
                _ => {}
            }
        }
        None
    }

    fn exit_plan_mode_plan_for(&self, id: &ToolUseId) -> Option<String> {
        self.entries.iter().find_map(|entry| {
            if let LogEntry::ToolStart {
                id: oid,
                name,
                arguments,
            } = entry
            {
                if oid == id && name == "exit_plan_mode" {
                    return arguments
                        .get("plan")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
            }
            None
        })
    }

    fn glob_pattern_for(&self, id: &ToolUseId) -> Option<String> {
        self.entries.iter().rev().find_map(|entry| {
            if let LogEntry::ToolStart {
                id: oid,
                name,
                arguments,
            } = entry
            {
                if oid == id && name == "glob" {
                    return arguments
                        .get("pattern")
                        .or_else(|| arguments.get("glob_pattern"))
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
            }
            None
        })
    }

    fn bash_mode_command_for(&self, id: &ToolUseId) -> Option<String> {
        let mut after_output = false;
        for entry in self.entries.iter().rev() {
            if let LogEntry::BashModeOutput { id: oid, .. } = entry {
                if oid == id {
                    after_output = true;
                }
                continue;
            }
            if after_output {
                if let LogEntry::BashModeInput { command } = entry {
                    return Some(command.clone());
                }
                break;
            }
        }
        None
    }

    fn toggle_scroll_viewer(&mut self, id: ToolUseId, viewer: ScrollViewerState) -> bool {
        if self
            .scroll_viewer
            .as_ref()
            .is_some_and(|v| v.tool_id() == &id)
        {
            self.scroll_viewer = None;
            return true;
        }
        self.scroll_viewer = Some(viewer);
        true
    }

    /// Replie automatiquement le dernier bloc `internal_reasoning` fermé.
    pub fn auto_collapse_internal_reasoning(&mut self) {
        let Some((idx, phase)) = self
            .entries
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, e)| match e {
                LogEntry::PhaseOpen { phase } => Some((i, *phase)),
                _ => None,
            })
        else {
            return;
        };
        if phase == drox_engine::Phase::InternalReasoning {
            self.collapsed_phases.insert(idx);
            self.bump_log_revision();
        }
    }

    /// Bascule le dernier bloc phase dans le fil.
    pub fn toggle_latest_phase_block(&mut self) -> bool {
        let Some(idx) = self.entries.iter().enumerate().rev().find_map(|(i, e)| {
            if matches!(e, LogEntry::PhaseOpen { .. }) {
                Some(i)
            } else {
                None
            }
        }) else {
            return false;
        };
        if self.collapsed_phases.contains(&idx) {
            self.collapsed_phases.remove(&idx);
        } else {
            self.collapsed_phases.insert(idx);
        }
        self.bump_log_revision();
        true
    }

    #[must_use]
    pub fn flattened_log_lines(&mut self) -> Vec<Line<'static>> {
        self.flattened_log_lines_internal(None)
    }

    #[must_use]
    pub fn flattened_log_lines_display(&mut self) -> Vec<Line<'static>> {
        let search = self.transcript_search.clone();
        self.flattened_log_lines_internal(search.as_ref())
    }

    fn ensure_log_cache(&mut self) {
        let key = crate::view::log_cache::cache_key(
            self.log_revision,
            &self.entries,
            &self.expanded_tools,
            &self.collapsed_phases,
        );
        if self
            .log_render_cache
            .as_ref()
            .is_some_and(|c| c.key == key)
        {
            return;
        }
        self.log_render_cache = Some(crate::view::log_cache::LogRenderCache::build(
            self.log_revision,
            &self.entries,
            &self.expanded_tools,
            &self.collapsed_phases,
        ));
    }

    fn flattened_log_lines_internal(
        &mut self,
        search: Option<&TranscriptSearchState>,
    ) -> Vec<Line<'static>> {
        self.ensure_log_cache();
        let mut lines = self
            .log_render_cache
            .as_ref()
            .map(|c| c.lines.clone())
            .unwrap_or_default();
        if let Some(ref s) = self.streaming {
            if !s.trim().is_empty() {
                let thinking = self.active_phase == Some(drox_engine::Phase::InternalReasoning);
                lines.extend(render::render_streaming(s, thinking));
            }
        }
        if self.phase == AppPhase::Running {
            lines.extend(crate::view::run_spinner::activity_lines(
                self.ui_frame_tick,
                self.run_started,
                self.active_phase,
                self.streaming.as_ref().is_some_and(|s| !s.trim().is_empty()),
                self.active_bash.is_some(),
            ));
        }
        if let Some(ref bash) = self.active_bash {
            lines.extend(bash_progress::progress_lines(bash, self.ui_frame_tick));
        }
        if let Some(ref hook) = self.hook_progress {
            lines.extend(hook_progress::progress_lines(hook, self.ui_frame_tick));
        }
        let Some(search) = search else {
            return lines;
        };
        if search.query.is_empty() {
            return lines;
        }
        let current = search.current_line_index();
        lines
            .iter()
            .enumerate()
            .map(|(idx, line)| {
                let is_current = current == Some(idx);
                let has_match = search.match_lines.contains(&idx);
                if has_match {
                    highlight_line(line, &search.query, is_current)
                } else {
                    line.clone()
                }
            })
            .collect()
    }
}

fn parse_todo_items(output: &Value) -> Option<Vec<TodoItemView>> {
    let arr = output.get("todos")?.as_array()?;
    let mut items = Vec::new();
    for v in arr {
        let id = v.get("id").and_then(Value::as_str)?.to_string();
        let content = v.get("content").and_then(Value::as_str)?.to_string();
        let status = v
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("pending")
            .to_string();
        items.push(TodoItemView {
            id,
            content,
            status,
        });
    }
    Some(items)
}

fn parse_course_plan(output: &Value) -> Option<(String, Vec<CourseStepView>, String)> {
    let title = output
        .get("courseTitle")
        .or_else(|| output.get("course_title"))
        .and_then(Value::as_str)?
        .to_string();
    let steps_arr = output.get("steps")?.as_array()?;
    let mut steps = Vec::new();
    for s in steps_arr {
        steps.push(CourseStepView {
            id: s.get("id").and_then(Value::as_str).unwrap_or("?").to_string(),
            title: s.get("title").and_then(Value::as_str).unwrap_or("?").to_string(),
            kind: s
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("lesson")
                .to_string(),
            status: s
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("pending")
                .to_string(),
        });
    }
    let summary = output
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some((title, steps, summary))
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
