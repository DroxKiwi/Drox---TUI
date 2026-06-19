//! Modal premier lancement (`/onboarding`).

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::AppState;

const STEPS: &[&str] = &[
    "Connexion IA — Ctrl+Shift+L ou `/server` : adresse Ollama, test de connexion, choix du modèle. Obligatoire avant d'envoyer un message.",
    "Workspace — Ctrl+Shift+W ou `/workspace` : changer le dossier de travail (nouvelle session). `/add-dir` ajoute un dossier en plus pour la session.",
    "Bienvenue dans Drox TUI — REPL terminal branché sur le moteur Rust local.",
    "Workspace — vérifiez le chemin dans le header. Sans --apply, les écritures fichier sont simulées.",
    "Démarrage — /init puis /init run pour créer DROX.md et .drox/.",
    "Composer — Entrée envoie · Shift+Entrée nouvelle ligne · ! mode bash · @ fichiers.",
    "Navigation — / palette · Ctrl+F fil · Ctrl+R historique · e viewer outil.",
    "Sessions — /sessions · /resume ses_… · mémoire /search · /rewind.",
    "Personnalisation — /theme · /color · /vim · /keybindings init · Ctrl+Shift+L connexion IA.",
];

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(dialog) = state.onboarding.as_ref() else {
        return;
    };

    let popup_w = area.width.saturating_sub(4).min(68);
    let popup_h = 16u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);
    frame.render_widget(Clear, popup);

    let step = dialog.step.min(STEPS.len().saturating_sub(1));
    let lines = vec![
        Line::from(Span::styled(
            "Premier pas avec Drox",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("Étape {}/{}", step + 1, STEPS.len()),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(STEPS[step]),
        Line::from(""),
        Line::from(Span::styled(
            if step + 1 >= STEPS.len() {
                "Entrée terminer · Esc passer"
            } else {
                "Entrée suivant · Esc passer"
            },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Onboarding ")
                    .style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left),
        popup,
    );
}
