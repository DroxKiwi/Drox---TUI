//! Tool `lsp` — interroge le Language Server Protocol via le client (VS Code).
//!
//! Tool **hybride** : seul le schéma et la documentation vivent côté Rust.
//! L'implémentation réelle est déléguée au client (VS Code) qui appelle les
//! providers `vscode.executeXxxProvider` et `vscode.languages.getDiagnostics`,
//! lesquels relaient les requêtes aux language servers configurés
//! (`rust-analyzer`, `tsserver`, `pyright`, etc.).
//!
//! Si le tool est invoqué côté moteur sans `RemoteTool` (cas hors VS Code),
//! `execute` renvoie une `ToolError::Remote` explicite : il **doit** être
//! shadowé par le client pour fonctionner.
//!
//! Opérations exposées (V1) :
//! - `diagnostics` : erreurs / warnings courants (un fichier ou tout le
//!   workspace).
//! - `workspace_symbol` : recherche un symbole par nom dans tout le workspace
//!   (équivalent `Ctrl+T` dans VS Code).
//! - `definition` : "Aller à la définition" (`F12`).
//! - `references` : trouve toutes les utilisations d'un symbole.
//! - `hover` : documentation / signature à une position.
//!
//! Position : `{ line, character }` en **0-indexed** comme la spec LSP.
//! Alternative pour `definition` / `references` / `hover` : passer `symbol`
//! (chaîne) au lieu d'une position — le client cherchera la première
//! occurrence dans le fichier indiqué.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::tool::Tool;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename_all = "snake_case")]
pub enum LspOp {
    Diagnostics,
    WorkspaceSymbol,
    Definition,
    References,
    Hover,
}

// Les structs ci-dessous ne sont consommées qu'à des fins de schéma JSON et
// de validation côté tests : leur exécution réelle se fait côté client
// (extension VS Code) via `RemoteTool`. On désactive donc `dead_code` qui
// pointerait à tort des champs "non lus".
#[allow(dead_code)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LspPosition {
    /// Numéro de ligne (0-indexed, comme LSP).
    pub line: u32,
    /// Numéro de colonne (0-indexed, comme LSP).
    pub character: u32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LspInput {
    /// Opération à exécuter.
    pub op: LspOp,
    /// Chemin du fichier (absolu ou relatif au workspace). Requis pour
    /// `diagnostics` (sauf si on veut tout le workspace), `definition`,
    /// `references` et `hover`.
    #[serde(default)]
    pub path: Option<String>,
    /// Position 0-indexée dans le fichier (LSP standard). Utilisé par
    /// `definition` / `references` / `hover`. Alternative : `symbol`.
    #[serde(default)]
    pub position: Option<LspPosition>,
    /// Symbole textuel à localiser dans `path` puis utilisé comme position
    /// (première occurrence par regex `\bSYMBOL\b`). Utile quand on ne
    /// connaît pas la position exacte. Ignoré si `position` est fourni.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Pour `workspace_symbol` : motif de recherche (fuzzy côté serveur).
    #[serde(default)]
    pub query: Option<String>,
    /// Nombre maximum d'entrées renvoyées par le tool (déduplication faite
    /// côté client). Par défaut 50, plafond 500 imposé côté client.
    #[serde(default)]
    pub max_results: Option<usize>,
}

pub struct LspTool;

#[async_trait]
impl Tool for LspTool {
    fn name(&self) -> &str {
        "lsp"
    }

    fn description(&self) -> &str {
        "Interroge le Language Server Protocol via VS Code (diagnostics, \
         définition, références, hover, recherche de symboles). \
         Recherche **sémantique** (par symbole) plutôt que textuelle comme \
         `grep`. Nécessite un language server actif côté éditeur \
         (rust-analyzer, tsserver, pyright…). \
         Opérations : `diagnostics` | `workspace_symbol` | `definition` | \
         `references` | `hover`. Positions en 0-indexed (LSP). Pour les \
         opérations à position, fournis soit `position`, soit `symbol` \
         (le client cherchera la première occurrence dans `path`)."
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schemars::schema_for!(LspInput)).unwrap_or(Value::Null)
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, _ctx: &ToolContext, _input: Value) -> Result<Value, ToolError> {
        Err(ToolError::remote(
            "`lsp` is implemented by the editor client; \
             run drox via the VS Code extension to enable LSP queries",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn schema_lists_all_operations() {
        let s = LspTool.input_schema();
        let s_str = s.to_string();
        for op in [
            "diagnostics",
            "workspace_symbol",
            "definition",
            "references",
            "hover",
        ] {
            assert!(
                s_str.contains(op),
                "schema must mention op `{op}`, got {s_str}"
            );
        }
    }

    #[test]
    fn parses_diagnostics_payload() {
        let v: LspInput = serde_json::from_value(json!({
            "op": "diagnostics",
            "path": "src/main.rs"
        }))
        .expect("valid payload");
        assert!(matches!(v.op, LspOp::Diagnostics));
        assert_eq!(v.path.as_deref(), Some("src/main.rs"));
    }

    #[test]
    fn parses_definition_with_position() {
        let v: LspInput = serde_json::from_value(json!({
            "op": "definition",
            "path": "src/lib.rs",
            "position": { "line": 12, "character": 4 }
        }))
        .expect("valid payload");
        assert!(matches!(v.op, LspOp::Definition));
        let pos = v.position.expect("position present");
        assert_eq!(pos.line, 12);
        assert_eq!(pos.character, 4);
    }

    #[test]
    fn parses_definition_with_symbol() {
        let v: LspInput = serde_json::from_value(json!({
            "op": "references",
            "path": "src/lib.rs",
            "symbol": "parse_input",
            "max_results": 50
        }))
        .expect("valid payload");
        assert!(matches!(v.op, LspOp::References));
        assert_eq!(v.symbol.as_deref(), Some("parse_input"));
    }

    #[tokio::test]
    async fn execute_returns_remote_error_when_not_shadowed() {
        let tool = LspTool;
        let ctx = ToolContext::new(camino::Utf8PathBuf::from("/tmp"), false);
        let err = tool
            .execute(&ctx, json!({ "op": "diagnostics" }))
            .await
            .expect_err("must error when no client handles `lsp`");
        assert!(
            matches!(err, ToolError::Remote(_)),
            "expected Remote, got {err:?}"
        );
    }
}
