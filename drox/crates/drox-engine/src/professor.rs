//! Gates mode **Professeur** — mutations autorisées uniquement pendant une
//! étape `exercise` / `checkpoint` active, sur les chemins du `workArea`.

use serde_json::Value;

/// Outils soumis à la gate pédagogique (hors `course_plan_write`).
pub const PROFESSOR_GATED_TOOLS: &[&str] = &[
    "file_edit",
    "file_write",
    "notebook_edit",
    "delete_path",
    "copy_path",
    "bash",
];

/// État dérivé du dernier `course_plan_write` réussi dans le run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfessorCourseState {
    pub has_plan: bool,
    /// `lesson` | `exercise` | `checkpoint` pour l'étape `active`.
    pub active_step_kind: Option<String>,
    /// Chemins autorisés (primary + reference du `workArea` actif).
    pub allowed_paths: Vec<String>,
}

pub const MUTATING_BEFORE_COURSE_PLAN: &str = "Blocked (Professor mode): call `course_plan_write` \
    first with a full course plan (`courseTitle` + `steps`) before any mutating tool. \
    Co-build the plan with the learner (`ask_user_question` if needed).";

pub const MUTATING_NO_ACTIVE_EXERCISE: &str = "Blocked (Professor mode): mutating tools are only \
    allowed during an **active** `exercise` or `checkpoint` step. If you are teaching, use \
    `[phase: teach]` with short commented excerpts — do not edit project files. When the learner \
    is ready to practice, set that step to `active` via `course_plan_write` with a `workArea`.";

pub const MUTATING_PATH_OUTSIDE_WORK_AREA: &str = "Blocked (Professor mode): this path is outside \
    the active step's `workArea`. Update `course_plan_write` to widen `primaryPaths` / \
    `referencePaths`, or target a file listed there. Sandbox drafts under `.drox/learn/` are \
    always allowed during an active exercise.";

/// Met à jour l'état à partir du JSON renvoyé par `course_plan_write`.
pub fn state_from_course_plan_output(value: &Value) -> ProfessorCourseState {
    let Some(steps) = value.get("steps").and_then(Value::as_array) else {
        return ProfessorCourseState {
            has_plan: true,
            ..Default::default()
        };
    };

    let mut active_step_kind = None;
    let mut allowed_paths = Vec::new();

    for step in steps {
        let status = step
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("");
        if status != "active" {
            continue;
        }
        active_step_kind = step
            .get("kind")
            .and_then(Value::as_str)
            .map(str::to_string);
        let wa = step.get("workArea").or_else(|| step.get("work_area"));
        if let Some(wa) = wa {
            for key in [
                "primaryPaths",
                "primary_paths",
                "referencePaths",
                "reference_paths",
            ] {
                if let Some(arr) = wa.get(key).and_then(Value::as_array) {
                    for p in arr.iter().filter_map(|v| v.as_str()) {
                        let t = p.trim();
                        if !t.is_empty() {
                            allowed_paths.push(t.to_string());
                        }
                    }
                }
            }
        }
        break;
    }

    ProfessorCourseState {
        has_plan: true,
        active_step_kind,
        allowed_paths,
    }
}

/// `Some(message)` si l'appel doit être refusé.
#[must_use]
pub fn check_mutating_tool(
    tool_name: &str,
    args: &Value,
    state: &ProfessorCourseState,
) -> Option<&'static str> {
    if !PROFESSOR_GATED_TOOLS.contains(&tool_name) {
        return None;
    }

    if !state.has_plan {
        return Some(MUTATING_BEFORE_COURSE_PLAN);
    }

    let kind = state.active_step_kind.as_deref()?;
    if kind != "exercise" && kind != "checkpoint" {
        return Some(MUTATING_NO_ACTIVE_EXERCISE);
    }

    // `bash` : pas de validation de chemin en V1 (compile/test dans l'exercice).
    if tool_name == "bash" {
        return None;
    }

    let paths = paths_from_tool_args(tool_name, args);
    if paths.is_empty() {
        return Some(MUTATING_PATH_OUTSIDE_WORK_AREA);
    }

    if paths
        .iter()
        .all(|p| path_allowed_in_work_area(p, &state.allowed_paths))
    {
        None
    } else {
        Some(MUTATING_PATH_OUTSIDE_WORK_AREA)
    }
}

#[must_use]
pub fn path_allowed_in_work_area(target: &str, allowed: &[String]) -> bool {
    let t = normalize_path(target);
    if t.is_empty() {
        return false;
    }
    if t == ".drox/learn" || t.starts_with(".drox/learn/") {
        return true;
    }
    if allowed.is_empty() {
        return false;
    }
    allowed.iter().any(|a| paths_match(&t, &normalize_path(a)))
}

fn paths_match(target: &str, allowed: &str) -> bool {
    if target == allowed {
        return true;
    }
    target.starts_with(&format!("{allowed}/"))
        || allowed.starts_with(&format!("{target}/"))
}

#[must_use]
pub fn normalize_path(p: &str) -> String {
    let mut s = p.replace('\\', "/").trim().to_string();
    while s.starts_with("./") {
        s = s[2..].to_string();
    }
    s.trim_start_matches('/').to_string()
}

fn paths_from_tool_args(tool_name: &str, args: &Value) -> Vec<String> {
    let mut out = Vec::new();
    match tool_name {
        "file_read" | "file_write" | "file_edit" | "notebook_edit" | "delete_path" => {
            if let Some(p) = args
                .get("path")
                .or_else(|| args.get("file_path"))
                .and_then(Value::as_str)
            {
                out.push(p.to_string());
            }
        }
        "copy_path" => {
            for key in ["source", "src", "from", "destination", "dest", "to"] {
                if let Some(p) = args.get(key).and_then(Value::as_str) {
                    out.push(p.to_string());
                }
            }
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn blocks_mutations_without_plan() {
        let state = ProfessorCourseState::default();
        assert_eq!(
            check_mutating_tool("file_edit", &json!({"path": "a.ts"}), &state),
            Some(MUTATING_BEFORE_COURSE_PLAN)
        );
    }

    #[test]
    fn blocks_mutations_during_lesson() {
        let state = ProfessorCourseState {
            has_plan: true,
            active_step_kind: Some("lesson".into()),
            allowed_paths: vec!["src/app/page.tsx".into()],
        };
        assert_eq!(
            check_mutating_tool("file_edit", &json!({"path": "src/app/page.tsx"}), &state),
            Some(MUTATING_NO_ACTIVE_EXERCISE)
        );
    }

    #[test]
    fn allows_exercise_path_in_work_area() {
        let state = ProfessorCourseState {
            has_plan: true,
            active_step_kind: Some("exercise".into()),
            allowed_paths: vec!["src/app/page.tsx".into()],
        };
        assert_eq!(
            check_mutating_tool(
                "file_edit",
                &json!({"path": "src/app/page.tsx", "edits": []}),
                &state
            ),
            None
        );
    }

    #[test]
    fn allows_drox_learn_sandbox() {
        let state = ProfessorCourseState {
            has_plan: true,
            active_step_kind: Some("exercise".into()),
            allowed_paths: vec![],
        };
        assert!(path_allowed_in_work_area(
            ".drox/learn/mission-1/draft.tsx",
            &state.allowed_paths
        ));
    }

    #[test]
    fn rejects_path_outside_work_area() {
        let state = ProfessorCourseState {
            has_plan: true,
            active_step_kind: Some("exercise".into()),
            allowed_paths: vec!["src/learn/page.tsx".into()],
        };
        assert_eq!(
            check_mutating_tool("file_write", &json!({"path": "src/other.ts"}), &state),
            Some(MUTATING_PATH_OUTSIDE_WORK_AREA)
        );
    }

    #[test]
    fn parses_course_plan_output() {
        let v = json!({
            "courseTitle": "CSS",
            "steps": [
                {"id":"1","kind":"lesson","status":"pending"},
                {"id":"2","kind":"exercise","status":"active",
                 "workArea":{"primaryPaths":["src/app/page.tsx"]}}
            ]
        });
        let s = state_from_course_plan_output(&v);
        assert!(s.has_plan);
        assert_eq!(s.active_step_kind.as_deref(), Some("exercise"));
        assert_eq!(s.allowed_paths, vec!["src/app/page.tsx"]);
    }
}
