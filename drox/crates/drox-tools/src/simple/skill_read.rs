//! Tool `skill_read` — charge le contenu complet d'un skill local.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::skills::{
    MAX_SKILL_BYTES_RETURNED, find_skill, load_skills_catalog, read_skill_file,
};
use crate::tool::Tool;

/// Payload reçu par le tool `skill_read`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SkillReadInput {
    /// Nom du skill (dossier sous `.drox/skills/` ou champ `name` du front-matter).
    pub name: String,
}

/// Tool local `skill_read`.
pub struct SkillReadTool;

#[async_trait]
impl Tool for SkillReadTool {
    fn name(&self) -> &str {
        "skill_read"
    }

    fn description(&self) -> &str {
        "Charge le fichier SKILL.md complet d'un skill local du workspace \
         (`.drox/skills/<name>/SKILL.md`). Lecture seule. À appeler avant \
         d'appliquer les instructions d'un skill listé au démarrage du run. \
         Format : {\"name\": \"…\"}."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(SkillReadInput)).unwrap_or(Value::Null)
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let args: SkillReadInput = serde_json::from_value(input).map_err(|e| {
            ToolError::invalid_args(format!(
                "skill_read: payload JSON invalide ({e}). Format attendu : {{\"name\": \"…\"}}.",
            ))
        })?;
        let name = args.name.trim();
        if name.is_empty() {
            return Err(ToolError::invalid_args("skill_read: `name` is empty"));
        }
        let catalog = load_skills_catalog(&ctx.workspace_root)
            .await
            .map_err(|e| ToolError::invalid_args(format!("skill_read: {e}")))?;
        let Some(entry) = find_skill(&catalog, name) else {
            return Err(ToolError::invalid_args(format!(
                "skill_read: no skill named `{name}` under `.drox/skills/`",
            )));
        };
        if entry.disable_model_invocation {
            return Err(ToolError::invalid_args(format!(
                "skill_read: skill `{name}` has disable-model-invocation — user-only",
            )));
        }
        let raw = read_skill_file(&entry.path)
            .await
            .map_err(|e| ToolError::io(entry.path.clone(), match e {
                crate::skills::SkillError::Io { source, .. } => source,
                crate::skills::SkillError::Other(m) => {
                    std::io::Error::new(std::io::ErrorKind::Other, m)
                }
            }))?;
        let (returned, truncated) = if raw.len() > MAX_SKILL_BYTES_RETURNED {
            (raw[..MAX_SKILL_BYTES_RETURNED].to_string(), true)
        } else {
            (raw, false)
        };
        Ok(json!({
            "name": entry.name,
            "path": entry.path.as_str(),
            "truncated": truncated,
            "content": returned,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use tempfile::tempdir;

    async fn seed(ws: &camino::Utf8Path, dir: &str, body: &str) {
        let p = ws.join(".drox/skills").join(dir);
        tokio::fs::create_dir_all(p.as_std_path()).await.unwrap();
        tokio::fs::write(p.join("SKILL.md").as_std_path(), body)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn reads_skill_content() {
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        seed(
            &ws,
            "commit",
            "---\ndescription: commit helper\n---\n\nRun tests first.",
        )
        .await;
        let ctx = ToolContext::new(ws, false);
        let out = SkillReadTool
            .execute(&ctx, json!({ "name": "commit" }))
            .await
            .unwrap();
        assert_eq!(out["name"], "commit");
        let content = out["content"].as_str().unwrap();
        assert!(content.contains("Run tests first"));
    }

    #[tokio::test]
    async fn rejects_disable_model_invocation() {
        let dir = tempdir().unwrap();
        let ws = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        seed(
            &ws,
            "deploy",
            "---\ndescription: x\ndisable-model-invocation: true\n---\n",
        )
        .await;
        let ctx = ToolContext::new(ws, false);
        let err = SkillReadTool
            .execute(&ctx, json!({ "name": "deploy" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidArgs(ref m) if m.contains("disable-model-invocation")));
    }
}
