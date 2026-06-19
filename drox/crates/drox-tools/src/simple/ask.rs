//! Tool `ask_user_question` — délègue une (ou plusieurs) question(s) au client
//! via `UserAsker`. Sprint Questions bloquantes (§2.13 du backlog) :
//!
//! Deux formes acceptées pour l'input (déserialisation tolérante) :
//!
//! - **Multi (recommandé)** :
//!   ```json
//!   {
//!     "title": "Architecture du cache ?",
//!     "questions": [
//!       { "id": "store", "prompt": "Où stocker ?",
//!         "options": [{"id":"a","label":"Redis"},{"id":"b","label":"in-memory"}],
//!         "allow_multiple": false, "allow_free_text": true }
//!     ]
//!   }
//!   ```
//! - **Mono (legacy)** :
//!   ```json
//!   { "question": "OK pour push ?", "choices": ["oui","non"] }
//!   ```
//!
//! L'output suit la même symétrie : multi → `{ answers: [{id, optionIds, freeText, skipped}] }` ;
//! mono → `{ answer, indices }` (rétro-compat).

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::asker::UserQuestion;
use crate::context::ToolContext;
use crate::error::ToolError;
use crate::tool::Tool;

/// Exemple JSON minimal copiable (§2.21 — messages d'erreur + nudge anti-boucle).
pub const CANONICAL_ASK_JSON_EXAMPLE: &str =
    r#"{"questions":[{"prompt":"Votre question ?","options":[{"id":"a","label":"Option A"},{"id":"b","label":"Option B"}],"allowFreeText":true}]}"#;

/// Option `{ id, label }` pour une question à choix (forme enrichie).
/// `id` est ce qui revient dans `optionIds` côté réponse ; `label` est
/// l'affichage utilisateur. Une question sans `options` = réponse libre.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AskOption {
    pub id: String,
    pub label: String,
}

/// Forme enrichie d'une question (un item de la file 1/N).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AskQuestionItem {
    /// Id logique (renvoyé tel quel dans la réponse). Génère `q1`/`q2`/… si
    /// absent.
    #[serde(default)]
    pub id: Option<String>,
    /// Énoncé en clair, présenté à l'utilisateur.
    pub prompt: String,
    /// Options à choisir. Vide = pas d'options structurées → réponse libre
    /// uniquement.
    #[serde(default)]
    pub options: Vec<AskOption>,
    /// Permet plusieurs `option_ids` cochés simultanément (cases). Default
    /// `false` = bouton radio.
    #[serde(default)]
    pub allow_multiple: bool,
    /// Permet à l'utilisateur de saisir un complément libre **en plus** des
    /// options (Cursor → champ « Add more optional details »). Default
    /// `false`.
    #[serde(default)]
    pub allow_free_text: bool,
}

/// Forme enrichie de l'input.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AskUserQuestionMulti {
    /// Titre optionnel de la carte (groupe).
    #[serde(default)]
    pub title: Option<String>,
    pub questions: Vec<AskQuestionItem>,
}

/// Forme legacy mono-question (rétro-compat).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AskUserQuestionMono {
    pub question: String,
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default)]
    pub allow_multiple: bool,
}

/// Union des deux schémas. `serde(untagged)` discrimine sur la présence des
/// champs ; on documente le multi comme forme canonique dans `description()`
/// pour que le modèle l'utilise par défaut.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum AskUserQuestionInput {
    Multi(AskUserQuestionMulti),
    Mono(AskUserQuestionMono),
}

pub struct AskUserQuestionTool;

#[async_trait]
impl Tool for AskUserQuestionTool {
    fn name(&self) -> &str {
        "ask_user_question"
    }

    fn description(&self) -> &str {
        "Pose une ou plusieurs questions à l'humain et attend ses réponses. \
         Appelle via tool_calls natifs (pas de JSON dans le texte assistant). \
         Forme recommandée (exemple minimal) : \
         `{\"questions\":[{\"prompt\":\"Quel texte sur la page d'accueil ?\",\
         \"options\":[{\"id\":\"a\",\"label\":\"Court\"},{\"id\":\"b\",\"label\":\"Long\"}],\
         \"allowFreeText\":true}]}`. \
         Legacy mono : `{\"question\":\"…\",\"choices\":[\"A\",\"B\"]}`. \
         À utiliser dès qu'un doute non trivial change les actions à venir \
         (architecture, périmètre, choix de techno) avant toute mutation. \
         Le run est mis en pause jusqu'à réponse ; l'utilisateur peut skipper \
         une question (champ `skipped: true` dans la réponse correspondante)."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(AskUserQuestionInput)).unwrap_or(Value::Null)
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<Value, ToolError> {
        let normalized = normalize_input(input);
        let args: AskUserQuestionInput = serde_json::from_value(normalized).map_err(|e| {
            ToolError::invalid_args(invalid_payload_message(&format!(
                "payload JSON invalide ({e})"
            )))
        })?;
        let Some(asker) = ctx.user_asker.clone() else {
            return Err(ToolError::interactive(
                "no UserAsker configured for this session",
            ));
        };

        match args {
            AskUserQuestionInput::Multi(multi) => {
                if multi.questions.is_empty() {
                    return Err(ToolError::invalid_args(
                        "`questions` must contain at least one item",
                    ));
                }
                let questions: Vec<UserQuestion> = multi
                    .questions
                    .iter()
                    .enumerate()
                    .map(|(i, q)| {
                        if q.prompt.trim().is_empty() {
                            return Err(ToolError::invalid_args(format!(
                                "question[{i}].prompt must not be empty"
                            )));
                        }
                        Ok(UserQuestion {
                            id: q
                                .id
                                .clone()
                                .or_else(|| Some(format!("q{}", i + 1))),
                            prompt: q.prompt.clone(),
                            choices: q
                                .options
                                .iter()
                                .map(|o| o.label.clone())
                                .collect(),
                            allow_multiple: q.allow_multiple,
                            allow_free_text: q.allow_free_text,
                        })
                    })
                    .collect::<Result<_, _>>()?;
                let answers = asker.ask_many(questions, multi.title).await?;

                let answers_json: Vec<Value> = answers
                    .into_iter()
                    .enumerate()
                    .map(|(i, a)| {
                        // Reconstruit `optionIds` depuis `indices` en se basant
                        // sur la liste d'options de la question d'origine,
                        // pour exposer au modèle des ids stables (et non des
                        // index numériques qui dépendraient de l'ordre).
                        let q = multi.questions.get(i);
                        let option_ids: Vec<String> = a
                            .indices
                            .iter()
                            .filter_map(|idx| {
                                q.and_then(|qq| qq.options.get(*idx))
                                    .map(|o| o.id.clone())
                            })
                            .collect();
                        json!({
                            "id": a.id.unwrap_or_else(|| format!("q{}", i + 1)),
                            "optionIds": option_ids,
                            "freeText": a.text,
                            "skipped": a.skipped,
                        })
                    })
                    .collect();

                Ok(json!({ "answers": answers_json }))
            }
            AskUserQuestionInput::Mono(mono) => {
                if mono.question.trim().is_empty() {
                    return Err(ToolError::invalid_args("question must not be empty"));
                }
                let answer = asker
                    .ask(UserQuestion {
                        id: None,
                        prompt: mono.question,
                        choices: mono.choices,
                        allow_multiple: mono.allow_multiple,
                        // Le schéma legacy n'expose pas le toggle ; on garde
                        // le comportement historique (texte libre toujours
                        // possible côté UI quand pas de choix structurés).
                        allow_free_text: false,
                    })
                    .await?;
                Ok(json!({
                    "answer": answer.text,
                    "indices": answer.indices,
                    "skipped": answer.skipped,
                }))
            }
        }
    }
}

/// Rattrape les payloads JSON mal formés que les LLM locaux envoient souvent.
fn normalize_input(input: Value) -> Value {
    use serde_json::map::Map;

    match input {
        Value::Array(items) => {
            let questions: Vec<Value> = items
                .into_iter()
                .map(|item| match item {
                    Value::Object(m) => normalize_question_item(m),
                    Value::String(s) => json!({ "prompt": s }),
                    other => other,
                })
                .collect();
            json!({ "questions": questions })
        }
        Value::Object(mut obj) => {
            remap_snake_keys(&mut obj);
            normalize_questions_field(&mut obj);

            if obj.contains_key("questions") {
                return Value::Object(obj);
            }

            // Legacy mono : champ racine `question` (string).
            if obj.get("question").and_then(|v| v.as_str()).is_some() {
                if let Some(c) = obj.remove("choices").or_else(|| obj.remove("options")) {
                    obj.insert("choices".into(), choices_labels_from_value(c));
                }
                return Value::Object(obj);
            }

            // Objet = une seule question (prompt/question à la racine).
            if obj.contains_key("prompt")
                || obj.contains_key("question")
                || obj.contains_key("text")
            {
                let title = obj.remove("title");
                let item = normalize_question_item(obj);
                let mut out = Map::new();
                if let Some(t) = title {
                    out.insert("title".into(), t);
                }
                out.insert("questions".into(), Value::Array(vec![item]));
                return Value::Object(out);
            }

            Value::Object(obj)
        }
        other => other,
    }
}

#[must_use]
fn invalid_payload_message(detail: &str) -> String {
    format!(
        "ask_user_question: {detail}. Appelle via tool_calls natifs (ne colle pas le JSON \
         dans le texte assistant). Exemple minimal valide : {CANONICAL_ASK_JSON_EXAMPLE}. \
         Legacy mono : {{\"question\":\"…\",\"choices\":[\"A\",\"B\"]}}."
    )
}

fn normalize_questions_field(obj: &mut serde_json::Map<String, Value>) {
    let Some(raw) = obj.remove("questions") else {
        return;
    };
    let questions = match raw {
        Value::String(s) => json!([{ "prompt": s }]),
        Value::Object(m) => Value::Array(vec![normalize_question_item(m)]),
        Value::Array(arr) => Value::Array(
            arr.into_iter()
                .map(|item| match item {
                    Value::Object(m) => normalize_question_item(m),
                    Value::String(s) => json!({ "prompt": s }),
                    other => other,
                })
                .collect(),
        ),
        other => Value::Array(vec![other]),
    };
    obj.insert("questions".into(), questions);
}

fn remap_snake_keys(obj: &mut serde_json::Map<String, Value>) {
    for (snake, camel) in [
        ("allow_multiple", "allowMultiple"),
        ("allow_free_text", "allowFreeText"),
        ("multi_select", "allowMultiple"),
    ] {
        if let Some(v) = obj.remove(snake) {
            obj.entry(camel.to_string()).or_insert(v);
        }
    }
    if let Some(v) = obj.remove("multiSelect") {
        obj.entry("allowMultiple".to_string()).or_insert(v);
    }
}

fn normalize_question_item(mut obj: serde_json::Map<String, Value>) -> Value {
    remap_snake_keys(&mut obj);

    if !obj.contains_key("id") {
        if let Some(h) = obj.remove("header") {
            if let Some(s) = h.as_str() {
                obj.insert("id".into(), json!(option_id_from_label(0, s)));
            } else {
                obj.insert("id".into(), h);
            }
        }
    }

    if !obj.contains_key("prompt") {
        if let Some(q) = obj.remove("question") {
            obj.insert("prompt".into(), q);
        } else if let Some(t) = obj.remove("text") {
            obj.insert("prompt".into(), t);
        }
    }

    if !obj.contains_key("options") {
        if let Some(c) = obj.remove("choices") {
            obj.insert("options".into(), options_from_choices_value(c));
        }
    } else if let Some(opts) = obj.remove("options") {
        obj.insert("options".into(), options_from_choices_value(opts));
    }

    Value::Object(obj)
}

/// `choices: ["A","B"]` ou `options: ["A","B"]` ou `options: [{label}]` → `[{id,label}]`.
fn options_from_choices_value(val: Value) -> Value {
    let Value::Array(arr) = val else {
        return Value::Array(vec![]);
    };
    Value::Array(
        arr.into_iter()
            .enumerate()
            .map(|(i, item)| match item {
                Value::String(s) => {
                    let id = option_id_from_label(i, &s);
                    json!({ "id": id, "label": s })
                }
                Value::Object(mut o) => {
                    remap_snake_keys(&mut o);
                    if !o.contains_key("id") {
                        let label = o
                            .get("label")
                            .or(o.get("text"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("option")
                            .to_string();
                        o.insert(
                            "id".into(),
                            json!(option_id_from_label(i, &label)),
                        );
                        if !o.contains_key("label") {
                            o.insert("label".into(), json!(label));
                        }
                    }
                    Value::Object(o)
                }
                other => other,
            })
            .collect(),
    )
}

/// Pour le schéma legacy mono : `choices` doit rester un tableau de chaînes.
fn choices_labels_from_value(val: Value) -> Value {
    let Value::Array(arr) = options_from_choices_value(val) else {
        return Value::Array(vec![]);
    };
    Value::Array(
        arr.into_iter()
            .filter_map(|o| o.get("label").cloned())
            .collect(),
    )
}

fn option_id_from_label(index: usize, label: &str) -> String {
    let slug: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let slug = slug.trim_matches('_');
    if slug.is_empty() {
        format!("opt{}", index + 1)
    } else {
        slug.chars().take(24).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asker::{UserAnswer, UserAsker};
    use std::sync::Arc;
    use std::sync::Mutex;

    /// Asker bouchon : enregistre les questions reçues, renvoie une réponse
    /// scriptée par index. Utilisé pour les tests des deux schémas (mono /
    /// multi) sans dépendre du JSON-RPC.
    struct ScriptedAsker {
        received: Arc<Mutex<Vec<UserQuestion>>>,
        scripted: Vec<UserAnswer>,
    }

    #[async_trait::async_trait]
    impl UserAsker for ScriptedAsker {
        async fn ask(&self, question: UserQuestion) -> Result<UserAnswer, ToolError> {
            self.received.lock().unwrap().push(question);
            // Pour `ask` mono on retourne la 1re réponse scriptée.
            Ok(self
                .scripted
                .get(0)
                .cloned()
                .unwrap_or(UserAnswer {
                    id: None,
                    text: String::new(),
                    indices: Vec::new(),
                    skipped: false,
                }))
        }

        async fn ask_many(
            &self,
            questions: Vec<UserQuestion>,
            _title: Option<String>,
        ) -> Result<Vec<UserAnswer>, ToolError> {
            let mut answers = Vec::new();
            for (i, q) in questions.iter().enumerate() {
                self.received.lock().unwrap().push(q.clone());
                let mut a = self.scripted.get(i).cloned().unwrap_or(UserAnswer {
                    id: None,
                    text: String::new(),
                    indices: Vec::new(),
                    skipped: false,
                });
                if a.id.is_none() {
                    a.id = q.id.clone();
                }
                answers.push(a);
            }
            Ok(answers)
        }
    }

    fn ctx_with_asker(asker: ScriptedAsker) -> (ToolContext, Arc<Mutex<Vec<UserQuestion>>>) {
        let received = asker.received.clone();
        let ctx = ToolContext::new(
            camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            false,
        )
        .with_user_asker(Arc::new(asker));
        (ctx, received)
    }

    #[tokio::test]
    async fn mono_schema_round_trips_legacy_output() {
        let asker = ScriptedAsker {
            received: Arc::new(Mutex::new(Vec::new())),
            scripted: vec![UserAnswer {
                id: None,
                text: "oui".into(),
                indices: vec![0],
                skipped: false,
            }],
        };
        let (ctx, received) = ctx_with_asker(asker);
        let tool = AskUserQuestionTool;
        let out = tool
            .execute(
                &ctx,
                json!({ "question": "OK pour push ?", "choices": ["oui", "non"] }),
            )
            .await
            .expect("execute");

        // Le schéma legacy produit `answer` + `indices` + `skipped`, pas
        // `answers[]`. Anti-régression : si on rename / supprime un champ,
        // les clients legacy cassent.
        assert_eq!(out["answer"], json!("oui"));
        assert_eq!(out["indices"], json!([0]));
        assert_eq!(out["skipped"], json!(false));
        assert!(out.get("answers").is_none());

        let q = received.lock().unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].choices, vec!["oui", "non"]);
        assert!(q[0].id.is_none(), "mono → pas d'id auto-généré");
    }

    #[tokio::test]
    async fn multi_schema_passes_questions_and_maps_option_ids() {
        let asker = ScriptedAsker {
            received: Arc::new(Mutex::new(Vec::new())),
            scripted: vec![
                UserAnswer {
                    id: Some("store".into()),
                    text: "Redis".into(),
                    indices: vec![0],
                    skipped: false,
                },
                UserAnswer {
                    id: Some("ttl".into()),
                    text: String::new(),
                    indices: Vec::new(),
                    skipped: true,
                },
            ],
        };
        let (ctx, received) = ctx_with_asker(asker);
        let tool = AskUserQuestionTool;
        let out = tool
            .execute(
                &ctx,
                json!({
                    "title": "Cache",
                    "questions": [
                        {
                            "id": "store",
                            "prompt": "Où stocker ?",
                            "options": [
                                {"id": "redis", "label": "Redis"},
                                {"id": "mem", "label": "In-memory"}
                            ]
                        },
                        {
                            "id": "ttl",
                            "prompt": "TTL en secondes ?",
                            "allowFreeText": true
                        }
                    ]
                }),
            )
            .await
            .expect("execute");

        let answers = out["answers"].as_array().expect("answers array");
        assert_eq!(answers.len(), 2);

        // 1re réponse : option `redis` retrouvée à l'index 0.
        assert_eq!(answers[0]["id"], json!("store"));
        assert_eq!(answers[0]["optionIds"], json!(["redis"]));
        assert_eq!(answers[0]["skipped"], json!(false));

        // 2de réponse : skipée → optionIds vide, freeText vide, skipped true.
        assert_eq!(answers[1]["id"], json!("ttl"));
        assert_eq!(answers[1]["optionIds"], json!([]));
        assert_eq!(answers[1]["skipped"], json!(true));

        // Vérifie que `allow_free_text` est propagé au UserAsker (donc
        // disponible côté RPC asker pour piloter l'UI).
        let q = received.lock().unwrap();
        assert_eq!(q.len(), 2);
        assert_eq!(q[0].id.as_deref(), Some("store"));
        assert!(!q[0].allow_free_text, "store ne demandait pas de free text");
        assert!(q[1].allow_free_text, "ttl demandait `allowFreeText: true`");
    }

    #[tokio::test]
    async fn normalizes_question_alias_and_string_choices() {
        let asker = ScriptedAsker {
            received: Arc::new(Mutex::new(Vec::new())),
            scripted: vec![UserAnswer {
                id: Some("q1".into()),
                text: "Court".into(),
                indices: vec![0],
                skipped: false,
            }],
        };
        let (ctx, received) = ctx_with_asker(asker);
        let tool = AskUserQuestionTool;
        tool.execute(
            &ctx,
            json!({
                "questions": [
                    {
                        "question": "Quel texte de présentation ?",
                        "choices": ["Court", "Long", "Autre"]
                    }
                ]
            }),
        )
        .await
        .expect("normalize question+choices");

        let q = received.lock().unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].prompt, "Quel texte de présentation ?");
        assert_eq!(q[0].choices, vec!["Court", "Long", "Autre"]);
    }

    #[tokio::test]
    async fn normalizes_root_array_of_questions() {
        let asker = ScriptedAsker {
            received: Arc::new(Mutex::new(Vec::new())),
            scripted: vec![UserAnswer {
                id: Some("q1".into()),
                text: String::new(),
                indices: vec![],
                skipped: false,
            }],
        };
        let (ctx, received) = ctx_with_asker(asker);
        let tool = AskUserQuestionTool;
        tool.execute(
            &ctx,
            json!([{ "prompt": "Une seule question en tableau racine ?" }]),
        )
        .await
        .expect("root array");

        assert_eq!(received.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn normalizes_root_prompt_object_to_multi() {
        let asker = ScriptedAsker {
            received: Arc::new(Mutex::new(Vec::new())),
            scripted: vec![UserAnswer {
                id: Some("q1".into()),
                text: "ok".into(),
                indices: vec![],
                skipped: false,
            }],
        };
        let (ctx, _) = ctx_with_asker(asker);
        let tool = AskUserQuestionTool;
        let out = tool
            .execute(
                &ctx,
                json!({
                    "prompt": "Où placer le bloc ?",
                    "options": ["Haut", "Bas"],
                    "allowFreeText": true
                }),
            )
            .await
            .expect("root prompt");
        assert!(out.get("answers").is_some());
    }

    #[tokio::test]
    async fn invalid_payload_returns_actionable_error() {
        let asker = ScriptedAsker {
            received: Arc::new(Mutex::new(Vec::new())),
            scripted: vec![],
        };
        let (ctx, _) = ctx_with_asker(asker);
        let tool = AskUserQuestionTool;
        let err = tool
            .execute(&ctx, json!({ "foo": 42 }))
            .await
            .expect_err("bad payload");
        let msg = err.to_string();
        assert!(msg.contains("ask_user_question"), "{msg}");
        assert!(msg.contains("questions"), "{msg}");
    }

    #[tokio::test]
    async fn normalizes_questions_as_string() {
        let asker = ScriptedAsker {
            received: Arc::new(Mutex::new(Vec::new())),
            scripted: vec![UserAnswer {
                id: Some("q1".into()),
                text: String::new(),
                indices: vec![],
                skipped: false,
            }],
        };
        let (ctx, received) = ctx_with_asker(asker);
        let tool = AskUserQuestionTool;
        tool.execute(
            &ctx,
            json!({ "questions": "Quelle méthode d'authentification ?" }),
        )
        .await
        .expect("questions string");

        assert_eq!(
            received.lock().unwrap()[0].prompt,
            "Quelle méthode d'authentification ?"
        );
    }

    #[tokio::test]
    async fn normalizes_leak_question_header_and_multiselect_aliases() {
        let asker = ScriptedAsker {
            received: Arc::new(Mutex::new(Vec::new())),
            scripted: vec![UserAnswer {
                id: Some("cache".into()),
                text: "Redis".into(),
                indices: vec![0],
                skipped: false,
            }],
        };
        let (ctx, received) = ctx_with_asker(asker);
        let tool = AskUserQuestionTool;
        tool.execute(
            &ctx,
            json!({
                "questions": [{
                    "question": "Stockage ?",
                    "header": "Cache",
                    "multiSelect": false,
                    "options": [
                        { "label": "Redis", "description": "Serveur Redis" },
                        { "label": "RAM", "description": "In-memory" }
                    ]
                }]
            }),
        )
        .await
        .expect("leak aliases");

        let q = received.lock().unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].prompt, "Stockage ?");
        assert_eq!(q[0].choices, vec!["Redis", "RAM"]);
        assert_eq!(q[0].id.as_deref(), Some("cache"));
    }

    #[tokio::test]
    async fn normalizes_single_question_object_at_root() {
        let asker = ScriptedAsker {
            received: Arc::new(Mutex::new(Vec::new())),
            scripted: vec![UserAnswer {
                id: Some("q1".into()),
                text: "ok".into(),
                indices: vec![],
                skipped: false,
            }],
        };
        let (ctx, _) = ctx_with_asker(asker);
        let tool = AskUserQuestionTool;
        let out = tool
            .execute(
                &ctx,
                json!({
                    "questions": { "prompt": "Une seule question en objet ?" }
                }),
            )
            .await
            .expect("questions object");
        assert!(out.get("answers").is_some());
    }

    #[tokio::test]
    async fn multi_schema_empty_questions_rejects() {
        let asker = ScriptedAsker {
            received: Arc::new(Mutex::new(Vec::new())),
            scripted: vec![],
        };
        let (ctx, _) = ctx_with_asker(asker);
        let tool = AskUserQuestionTool;
        let err = tool
            .execute(&ctx, json!({ "questions": [] }))
            .await
            .expect_err("empty questions must be rejected");
        match err {
            ToolError::InvalidArgs(msg) => assert!(msg.contains("must contain at least one")),
            other => panic!("expected InvalidArgs, got {other:?}"),
        }
    }
}
