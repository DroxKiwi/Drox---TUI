//! Menu d'aide composer (`?`) — raccourcis essentiels (leak : `PromptInputHelpMenu.tsx`).

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::AppState;

/// Lignes d'aide affichées dans la popup.
const HELP_LINES: &[&str] = &[
    "Ctrl+Shift+L  connexion IA (Ollama) · /server",
    "Ctrl+Shift+W  changer workspace · /workspace",
    "!          mode bash (shell sans agent)",
    "/          commandes slash · /help liste complète",
    "@          référence fichier · Tab compléter",
    "/skills    complétion nom de skill",
    "Tab        accepter suggestion · ↑↓ naviguer",
    "Ctrl+R     historique prompts · Ctrl+F recherche fil",
    "e          développer dernière sortie outil",
    "Ctrl+V     collage texte · image (chemin ou presse-papiers Win)",
    "/vim       mode vim composer (Esc INSERT/NORMAL)",
    "Esc        annuler · Ctrl+Q quitter",
];

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let h = (HELP_LINES.len() as u16 + 2).min(area.height.saturating_sub(2));
    let w = area.width.min(72);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h + 2);
    let popup = Rect::new(x, y, w, h);
    frame.render_widget(Clear, popup);

    let body: Vec<Line> = HELP_LINES
        .iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(state.palette.header_muted))))
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Aide composer (?) — Esc fermer ")
        .style(Style::default().fg(state.palette.composer_border));

    frame.render_widget(Paragraph::new(body).block(block).wrap(Wrap { trim: true }), popup);
}
