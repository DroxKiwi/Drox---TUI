//! Tool `skill_list` — liste les skills locaux du workspace.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::skills::{load_skills_catalog, model_visible_skills};
use crate::tool::Tool;

/// Payload reçu par le tool `skill_list` (vide ou `{}`).
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct SkillListInput {}

/// Tool local `skill_list`.
pub struct SkillListTool;

#[async_trait]
impl Tool for SkillListTool {
    fn name(&self) -> &str {
        "skill_list"
    }

    fn description(&self) -> &str {
        "Liste les skills locaux du workspace (`.drox/skills/*/SKILL.md`), \
         triés par nom. Lecture seule. Renvoie name, description, when_to_use \
         — pas le body (utilise `skill_read`). Format : {{}}."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(SkillListInput)).unwrap_or(Value::Null)
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        if !input.is_null() && !input.as_object().is_some_and(|o| o.is_empty()) {
            let _: SkillListInput = serde_json::from_value(input).map_err(|e| {
                ToolError::invalid_args(format!(
                    "skill_list: payload JSON invalide ({e}). Format attendu : {{}}."
                ))
            })?;
        }
        let catalog = load_skills_catalog(&ctx.workspace_root)
            .await
            .map_err(|e| ToolError::invalid_args(format!("skill_list: {e}")))?;
        let visible = model_visible_skills(&catalog);
        let items: Vec<Value> = visible
            .iter()
            .map(|e| {
                json!({
                    "name": e.name,
                    "description": e.description,
                    "when_to_use": e.when_to_use,
                    "path": e.path.as_str(),
                })
            })
            .collect();
        Ok(json!({
            "skills": items,
            "count": items.len(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn lists_visible_skills_only() {
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let base = ws.join(".drox/skills");
        for (name, extra) in [("a", ""), ("b", "disable-model-invocation: true\n")] {
            let p = base.join(name);
            tokio::fs::create_dir_all(p.as_std_path()).await.unwrap();
            let md = format!("---\ndescription: {name}\n{extra}---\n");
            tokio::fs::write(p.join("SKILL.md").as_std_path(), md)
                .await
                .unwrap();
        }
        let ctx = ToolContext::new(ws, false);
        let out = SkillListTool.execute(&ctx, json!({})).await.unwrap();
        let skills = out["skills"].as_array().unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0]["name"], "a");
    }
}
