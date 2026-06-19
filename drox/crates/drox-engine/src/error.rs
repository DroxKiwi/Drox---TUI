//! Erreurs du moteur agent.

use drox_llm::LlmError;
use drox_tools::ToolError;
use thiserror::Error;

/// Erreur retournée par la boucle agent.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EngineError {
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),

    #[error("tool error: {0}")]
    Tool(#[from] ToolError),

    #[error("max iterations reached: {0}")]
    MaxIterations(usize),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("session / transcript: {0}")]
    Session(#[from] drox_session::SessionError),

    /// Sprint M1 — erreur de persistance / compaction de la mémoire de session.
    /// Wrapper textuel : ces échecs sont rares et leur message brut est plus
    /// utile à l'humain qu'une chaîne de types intermédiaires.
    #[error("memory: {0}")]
    Memory(String),

    /// Sprint Hotfix « boucle édition/lecture » — le modèle a émis trois tours
    /// consécutifs dont les empreintes (texte assistant nettoyé + signature
    /// des `tool_calls`) sont identiques après un nudge anti-boucle. On
    /// stoppe explicitement plutôt que de gaspiller des tokens jusqu'à
    /// `max_iterations`. Le `kind` décrit ce qui se répétait (« text »,
    /// « tool_calls », « both ») pour aider au debug.
    #[error("loop detected: model repeated the same {kind} for {turns} consecutive turns")]
    LoopDetected { kind: &'static str, turns: u32 },
}
