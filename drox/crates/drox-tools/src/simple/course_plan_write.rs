//! Tool `course_plan_write` — **plan de cours** pour le mode Professeur.
//!
//! Même modèle stateless que `todo_write` : le modèle envoie la liste
//! complète (replace), le tool valide et renvoie le payload normalisé.
//! L'extension VS Code affiche un bloc « Plan de cours » qui s'update en place.

use std::collections::HashSet;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::tool::Tool;

/// Type d'étape dans un plan de cours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename_all = "snake_case")]
pub enum CourseStepKind {
    /// Explication du sujet.
    Lesson,
    /// Travail pratique pour l'élève.
    Exercise,
    /// Contrôle / synthèse finale.
    Checkpoint,
}

/// Statut d'une étape du plan de cours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename_all = "snake_case")]
pub enum CourseStepStatus {
    Pending,
    /// Étape en cours — **au plus une** à la fois.
    Active,
    Mastered,
    Skipped,
}

/// Zone de travail suggérée pour un exercice (emplacement dans le repo).
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CourseWorkArea {
    #[serde(default)]
    pub strategy: Option<String>,
    #[serde(default)]
    pub primary_paths: Vec<String>,
    #[serde(default)]
    pub reference_paths: Vec<String>,
    #[serde(default)]
    pub rationale: Option<String>,
}

/// Une étape du plan de cours.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CourseStep {
    pub id: String,
    pub title: String,
    pub kind: CourseStepKind,
    pub status: CourseStepStatus,
    #[serde(default)]
    pub work_area: Option<CourseWorkArea>,
}

/// Payload `course_plan_write`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoursePlanWriteInput {
    /// Objectif pédagogique global (ex. « Comprendre les animations Next.js »).
    pub course_title: String,
    pub steps: Vec<CourseStep>,
}

pub struct CoursePlanWriteTool;

#[derive(Debug, Default, Serialize)]
struct CoursePlanCounts {
    pending: usize,
    active: usize,
    mastered: usize,
    skipped: usize,
}

impl CoursePlanCounts {
    fn from_steps(steps: &[CourseStep]) -> Self {
        let mut c = Self::default();
        for s in steps {
            match s.status {
                CourseStepStatus::Pending => c.pending += 1,
                CourseStepStatus::Active => c.active += 1,
                CourseStepStatus::Mastered => c.mastered += 1,
                CourseStepStatus::Skipped => c.skipped += 1,
            }
        }
        c
    }

    fn summary(&self, total: usize, title: &str) -> String {
        format!(
            "Plan de cours «{title}» : {total} étape(s) — {} pending · {} active · {} mastered · {} skipped",
            self.pending, self.active, self.mastered, self.skipped
        )
    }
}

#[async_trait]
impl Tool for CoursePlanWriteTool {
    fn name(&self) -> &str {
        "course_plan_write"
    }

    fn description(&self) -> &str {
        "Crée ou met à jour le **plan de cours** (mode Professeur). Liste \
         complète en mode replace. Chaque cours alterne enseignement + exercice ; \
         inclure un `checkpoint` final si pertinent.\n\n\
         Format : `{ \"courseTitle\": \"…\", \"steps\": [{ \"id\", \"title\", \
         \"kind\": \"lesson|exercise|checkpoint\", \"status\": \"pending|active|mastered|skipped\", \
         \"workArea\"?: { \"strategy\", \"primaryPaths\", \"referencePaths\", \"rationale\" } }] }`.\n\n\
         Au plus **une** étape `active`. Pour les `exercise` / `checkpoint`, \
         renseigne `workArea` (où l'élève travaille dans le repo ouvert)."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(CoursePlanWriteInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, _ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let normalized = normalize_input(input);
        let args: CoursePlanWriteInput = serde_json::from_value(normalized).map_err(|e| {
            ToolError::invalid_args(format!(
                "course_plan_write: payload JSON invalide ({e}). \
                 Format : {{\"courseTitle\":\"…\",\"steps\":[{{\"id\",\"title\",\"kind\",\"status\"}}]}}."
            ))
        })?;
        validate(&args)?;
        let counts = CoursePlanCounts::from_steps(&args.steps);
        let title = args.course_title.trim();
        let summary = counts.summary(args.steps.len(), title);
        Ok(json!({
            "courseTitle": title,
            "steps": args.steps,
            "counts": counts,
            "summary": summary,
        }))
    }
}

fn normalize_input(input: Value) -> Value {
    if let Some(steps) = input.get("steps").and_then(|v| v.as_array()) {
        if !input.get("courseTitle").is_some() && !input.get("course_title").is_some() {
            if let Some(title) = input.get("mission").and_then(|v| v.as_str()) {
                return json!({
                    "courseTitle": title,
                    "steps": steps,
                });
            }
        }
    }
    input
}

fn validate(args: &CoursePlanWriteInput) -> Result<(), ToolError> {
    let title = args.course_title.trim();
    if title.is_empty() {
        return Err(ToolError::invalid_args(
            "course_plan_write: `courseTitle` must not be empty",
        ));
    }
    if args.steps.is_empty() {
        return Err(ToolError::invalid_args(
            "course_plan_write needs at least one step",
        ));
    }
    let mut ids = HashSet::new();
    let mut active = 0u32;
    for (i, step) in args.steps.iter().enumerate() {
        let id = step.id.trim();
        if id.is_empty() {
            return Err(ToolError::invalid_args(format!(
                "course_plan_write: step #{i} has empty `id`"
            )));
        }
        if !ids.insert(id.to_string()) {
            return Err(ToolError::invalid_args(format!(
                "course_plan_write: duplicate id `{id}`"
            )));
        }
        if step.title.trim().is_empty() {
            return Err(ToolError::invalid_args(format!(
                "course_plan_write: step `{id}` has empty `title`"
            )));
        }
        if step.status == CourseStepStatus::Active {
            active += 1;
        }
        if matches!(step.kind, CourseStepKind::Exercise | CourseStepKind::Checkpoint) {
            let wa = step.work_area.as_ref();
            let has_paths = wa.is_some_and(|w| {
                !w.primary_paths.is_empty() || !w.reference_paths.is_empty()
            });
            if !has_paths {
                return Err(ToolError::invalid_args(format!(
                    "course_plan_write: step `{id}` ({:?}) requires `workArea` with \
                     `primaryPaths` and/or `referencePaths` (where the learner works in the repo)",
                    step.kind
                )));
            }
        }
    }
    if active > 1 {
        return Err(ToolError::invalid_args(
            "course_plan_write: only one step can be `active` at a time",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ToolContext {
        ToolContext::new(camino::Utf8PathBuf::from("."), false)
    }

    #[tokio::test]
    async fn accepts_minimal_plan() {
        let payload = json!({
            "courseTitle": "Animations Next.js",
            "steps": [
                { "id": "1", "title": "Cours CSS", "kind": "lesson", "status": "active" },
                {
                    "id": "2",
                    "title": "Exo CSS",
                    "kind": "exercise",
                    "status": "pending",
                    "workArea": { "primaryPaths": [".drox/learn/exo-css/"] }
                }
            ]
        });
        let out = CoursePlanWriteTool.execute(&ctx(), payload).await.unwrap();
        assert_eq!(out["courseTitle"], "Animations Next.js");
        assert_eq!(out["counts"]["active"], 1);
    }

    #[tokio::test]
    async fn rejects_exercise_without_work_area() {
        let payload = json!({
            "courseTitle": "Rust",
            "steps": [
                { "id": "1", "title": "Exo", "kind": "exercise", "status": "pending" }
            ]
        });
        let err = CoursePlanWriteTool
            .execute(&ctx(), payload)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("workArea"), "{}", err);
    }

    #[tokio::test]
    async fn rejects_two_active() {
        let payload = json!({
            "courseTitle": "X",
            "steps": [
                { "id": "1", "title": "A", "kind": "lesson", "status": "active" },
                { "id": "2", "title": "B", "kind": "lesson", "status": "active" }
            ]
        });
        assert!(CoursePlanWriteTool.execute(&ctx(), payload).await.is_err());
    }
}
