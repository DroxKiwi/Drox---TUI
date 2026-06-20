//! Animation d'accueil au lancement — dissolve pixelisé phosphore.

use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use ratatui::Terminal;

use crate::ui::theme::drox;
use crate::ui::ThemePalette;

const FRAMES: u8 = 30;
const FRAME_MS: u64 = 42;

static LOGO: &[&str] = &[
    "██████╗ ██████╗  ██████╗ ██╗  ██╗",
    "██╔══██╗██╔══██╗██╔═══██╗╚██╗██╔╝",
    "██║  ██║██████╔╝██║   ██║ ╚███╔╝ ",
    "██║  ██║██╔══██╗██║   ██║ ██╔██╗ ",
    "██████╔╝██║  ██║╚██████╔╝██╔╝ ██╗",
    "╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═╝",
];

static LOGO_COMPACT: &str = "DROX TUI";

/// Joue l'intro puis retourne (ignorable avec une touche).
pub fn play<B: ratatui::backend::Backend>(
    term: &mut Terminal<B>,
    palette: &ThemePalette,
    animations_enabled: bool,
) -> std::io::Result<()> {
    if !animations_enabled {
        term.draw(|f| render(f, palette, FRAMES - 1, FRAMES))?;
        return Ok(());
    }

    for tick in 0..FRAMES {
        term.draw(|f| render(f, palette, tick, FRAMES))?;
        thread::sleep(Duration::from_millis(FRAME_MS));
        while event::poll(Duration::from_millis(0))? {
            match event::read()? {
                Event::Key(_) | Event::Mouse(_) => return Ok(()),
                _ => {}
            }
        }
    }
    Ok(())
}

fn render(frame: &mut Frame, palette: &ThemePalette, tick: u8, total: u8) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(palette.bg)),
        area,
    );

    let progress = (tick as f32 + 1.0) / total as f32;
    let glow_phase = (tick as f32 / total as f32).clamp(0.0, 1.0);

    if area.width >= 36 && area.height >= 14 {
        render_pixel_field(frame, area, palette, progress, glow_phase);
        render_logo_block(frame, area, palette, progress);
    } else {
        render_compact(frame, area, palette, progress);
    }

    if progress > 0.55 {
        let tagline = if progress > 0.88 {
            "initialisation…"
        } else {
            "terminal agent"
        };
        let y = area.height.saturating_sub(3).max(1);
        let tag_rect = Rect {
            x: area.x,
            y: area.y + y,
            width: area.width,
            height: 1,
        };
        let alpha = ((progress - 0.55) / 0.35).clamp(0.0, 1.0);
        let fg = lerp_color(palette.header_muted, palette.accent_glow, alpha);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                tagline,
                Style::default().fg(fg),
            )))
            .alignment(Alignment::Center),
            tag_rect,
        );
    }
}

fn render_pixel_field(frame: &mut Frame, area: Rect, palette: &ThemePalette, progress: f32, glow: f32) {
    let mut lines = Vec::new();
    for y in 0..area.height {
        let mut spans = Vec::new();
        for x in 0..area.width {
            let hash = cell_hash(x, y);
            let reveal = cell_reveal(progress, hash);
            if reveal <= 0.02 {
                spans.push(Span::styled(
                    " ",
                    Style::default().bg(palette.bg),
                ));
                continue;
            }
            let ch = pixel_char(reveal);
            let fg = pixel_color(palette, reveal, glow);
            spans.push(Span::styled(
                ch.to_string(),
                Style::default().fg(fg).bg(palette.bg),
            ));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_logo_block(frame: &mut Frame, area: Rect, palette: &ThemePalette, progress: f32) {
    let logo_h = LOGO.len() as u16;
    let logo_w = LOGO.first().map(|l| l.chars().count()).unwrap_or(0) as u16;
    let x = area.x + area.width.saturating_sub(logo_w) / 2;
    let y = area.y + area.height.saturating_sub(logo_h + 4) / 2;

    let logo_progress = ((progress - 0.25) / 0.55).clamp(0.0, 1.0);
    if logo_progress <= 0.0 {
        return;
    }

    let mut lines = Vec::new();
    for (row, line) in LOGO.iter().enumerate() {
        let row_reveal = ((logo_progress * LOGO.len() as f32) - row as f32).clamp(0.0, 1.0);
        if row_reveal <= 0.0 {
            continue;
        }
        let style = if row_reveal >= 0.95 {
            Style::default()
                .fg(palette.accent_glow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(lerp_color(palette.text_muted, palette.accent_bright, row_reveal))
        };
        lines.push(Line::from(Span::styled(*line, style)));
    }

    let block = Rect {
        x,
        y,
        width: logo_w.min(area.width),
        height: lines.len() as u16,
    };
    frame.render_widget(Paragraph::new(lines), block);
}

fn render_compact(frame: &mut Frame, area: Rect, palette: &ThemePalette, progress: f32) {
    render_pixel_field(frame, area, palette, progress * 0.9, progress);
    let logo_progress = ((progress - 0.2) / 0.6).clamp(0.0, 1.0);
    if logo_progress <= 0.0 {
        return;
    }
    let style = Style::default()
        .fg(lerp_color(
            palette.text_muted,
            palette.accent_glow,
            logo_progress,
        ))
        .add_modifier(Modifier::BOLD);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(LOGO_COMPACT, style)))
            .alignment(Alignment::Center),
        Rect {
            x: area.x,
            y: area.y + area.height / 2,
            width: area.width,
            height: 1,
        },
    );
}

#[must_use]
fn cell_hash(x: u16, y: u16) -> u16 {
    let n = (x as u32)
        .wrapping_mul(73_856_093)
        .wrapping_add((y as u32).wrapping_mul(19_349_663));
    (n % 997) as u16
}

#[must_use]
fn cell_reveal(progress: f32, hash: u16) -> f32 {
    let threshold = hash as f32 / 997.0;
    let eased = progress * progress;
    ((eased - threshold) * 3.2).clamp(0.0, 1.0)
}

#[must_use]
fn pixel_char(reveal: f32) -> char {
    if reveal < 0.2 {
        '·'
    } else if reveal < 0.45 {
        '░'
    } else if reveal < 0.7 {
        '▒'
    } else if reveal < 0.9 {
        '▓'
    } else {
        '█'
    }
}

#[must_use]
fn pixel_color(palette: &ThemePalette, reveal: f32, glow: f32) -> Color {
    if reveal < 0.35 {
        return drox::BG_ELEVATED;
    }
    if reveal < 0.65 {
        return lerp_color(drox::BG_ELEVATED, palette.text_muted, reveal);
    }
    lerp_color(palette.text_muted, palette.accent_glow, reveal * glow)
}

#[must_use]
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => Color::Rgb(
            lerp_u8(r1, r2, t),
            lerp_u8(g1, g2, t),
            lerp_u8(b1, b2, t),
        ),
        _ => if t < 0.5 { a } else { b },
    }
}

#[must_use]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_spreads_with_progress() {
        let low = cell_reveal(0.2, 100);
        let high = cell_reveal(0.9, 100);
        assert!(high > low);
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(cell_hash(3, 7), cell_hash(3, 7));
    }
}
