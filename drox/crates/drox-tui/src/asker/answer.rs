//! Parsing des réponses utilisateur (aligné `drox-cli` stdin).

use drox_tools::{UserAnswer, UserQuestion};

pub fn parse_answer(question: &UserQuestion, response: String) -> UserAnswer {
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
            if (1..=question.choices.len()).contains(&n) {
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
