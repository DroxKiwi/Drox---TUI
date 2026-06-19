//! Implémentation `UserAsker` pour le binaire CLI.
//!
//! Lit la question sur stderr (pour ne pas polluer stdout du modèle) et la
//! réponse depuis stdin. Si la question propose des choix, accepte des indices
//! 1-based (`"1"` ou `"1,3"` si `allow_multiple`).

use async_trait::async_trait;
use drox_tools::{ToolError, UserAnswer, UserAsker, UserQuestion};
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};

pub struct StdinUserAsker;

#[async_trait]
impl UserAsker for StdinUserAsker {
    async fn ask(&self, question: UserQuestion) -> Result<UserAnswer, ToolError> {
        // NOTE: `#[async_trait]` aligne la lifetime de `ask` avec celle du trait.
        print_prompt(&question)?;

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();
        let read = reader
            .read_line(&mut line)
            .await
            .map_err(|e| ToolError::interactive(format!("stdin read error: {e}")))?;
        if read == 0 {
            return Err(ToolError::interactive("stdin closed before answer"));
        }
        let response = line.trim().to_string();
        Ok(parse_answer(&question, response))
    }
}

fn print_prompt(question: &UserQuestion) -> Result<(), ToolError> {
    let mut err = io::stderr().lock();
    writeln!(err, "\n[drox ?] {}", question.prompt)
        .map_err(|e| ToolError::interactive(format!("stderr write error: {e}")))?;
    if !question.choices.is_empty() {
        for (i, choice) in question.choices.iter().enumerate() {
            writeln!(err, "  {}) {choice}", i + 1)
                .map_err(|e| ToolError::interactive(format!("stderr write error: {e}")))?;
        }
        if question.allow_multiple {
            writeln!(err, "(indices séparés par virgule, ou texte libre)")
                .map_err(|e| ToolError::interactive(format!("stderr write error: {e}")))?;
        }
    }
    write!(err, "> ").map_err(|e| ToolError::interactive(format!("stderr write error: {e}")))?;
    err.flush()
        .map_err(|e| ToolError::interactive(format!("stderr flush error: {e}")))?;
    Ok(())
}

fn parse_answer(question: &UserQuestion, response: String) -> UserAnswer {
    if question.choices.is_empty() {
        return UserAnswer {
            id: question.id.clone(),
            text: response,
            indices: Vec::new(),
            skipped: false,
        };
    }

    let raw_parts: Vec<&str> = if question.allow_multiple {
        response.split(',').map(str::trim).collect()
    } else {
        vec![response.trim()]
    };

    let mut indices = Vec::new();
    for part in &raw_parts {
        if let Ok(n) = part.parse::<usize>() {
            if n >= 1 && n <= question.choices.len() {
                indices.push(n - 1);
            }
        }
    }
    indices.sort_unstable();
    indices.dedup();

    if indices.is_empty() {
        UserAnswer {
            id: question.id.clone(),
            text: response,
            indices: Vec::new(),
            skipped: false,
        }
    } else {
        let text = indices
            .iter()
            .map(|&i| question.choices[i].clone())
            .collect::<Vec<_>>()
            .join(",");
        UserAnswer {
            id: question.id.clone(),
            text,
            indices,
            skipped: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(prompt: &str, choices: &[&str], allow_multiple: bool) -> UserQuestion {
        UserQuestion {
            id: None,
            prompt: prompt.into(),
            choices: choices.iter().map(|c| (*c).to_string()).collect(),
            allow_multiple,
            allow_free_text: false,
        }
    }

    #[test]
    fn parse_free_text_when_no_choices() {
        let answer = parse_answer(&q("ok?", &[], false), "yes".into());
        assert_eq!(answer.text, "yes");
        assert!(answer.indices.is_empty());
    }

    #[test]
    fn parse_single_index() {
        let answer = parse_answer(&q("pick", &["a", "b", "c"], false), "2".into());
        assert_eq!(answer.indices, vec![1]);
        assert_eq!(answer.text, "b");
    }

    #[test]
    fn parse_multi_indices() {
        let answer = parse_answer(&q("pick", &["a", "b", "c"], true), "1, 3".into());
        assert_eq!(answer.indices, vec![0, 2]);
        assert_eq!(answer.text, "a,c");
    }

    #[test]
    fn unknown_index_falls_back_to_free_text() {
        let answer = parse_answer(&q("pick", &["a", "b"], false), "maybe".into());
        assert!(answer.indices.is_empty());
        assert_eq!(answer.text, "maybe");
    }
}
