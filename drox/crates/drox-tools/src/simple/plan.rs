//! Tool `exit_plan_mode` — l'agent présente un plan finalisé et demande
//! l'autorisation de quitter le mode plan.
//!
//! Ce tool n'apporte aucune modification filesystem ; il transmet le plan
//! à l'humain via `UserAsker` et retourne sa décision. Le client (CLI ou
//! extension VS Code) est responsable de désactiver `plan_mode` côté contexte
//! pour les appels suivants si la réponse est positive.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::asker::UserQuestion;
use crate::context::ToolContext;
use crate::error::ToolError;
use crate::tool::Tool;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExitPlanModeInput {
    /// Plan textuel rédigé par l'agent (markdown autorisé).
    pub plan: String,
}

pub struct ExitPlanModeTool;

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        "exit_plan_mode"
    }

    fn description(&self) -> &str {
        "Présente un plan finalisé à l'humain et demande son accord pour \
         quitter le mode plan et passer à l'exécution."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(ExitPlanModeInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: ExitPlanModeInput = serde_json::from_value(input)?;
        if args.plan.trim().is_empty() {
            return Err(ToolError::invalid_args("plan must not be empty"));
        }
        let Some(asker) = ctx.user_asker.clone() else {
            return Err(ToolError::interactive(
                "no UserAsker configured for this session",
            ));
        };

        let prompt = format!("Plan proposé :\n\n{}\n\nL'approuves-tu ?", args.plan);
        let answer = asker
            .ask(UserQuestion {
                id: None,
                prompt,
                choices: vec![
                    "Oui, exécuter".to_string(),
                    "Non, rester en plan".to_string(),
                ],
                allow_multiple: false,
                allow_free_text: false,
            })
            .await?;

        let accepted = answer.indices.first().copied() == Some(0)
            || answer.text.eq_ignore_ascii_case("oui")
            || answer.text.eq_ignore_ascii_case("yes")
            || answer.text.eq_ignore_ascii_case("y");

        Ok(json!({
            "accepted": accepted,
            "user_response": answer.text,
        }))
    }
}
