//! Animation d'accueil au lancement — halo phosphore + logo DROX.

use std::thread;
use std::time::Duration;

use crossterm::event;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use ratatui::Terminal;

use crate::i18n::{self, keys};
use crate::ui::theme::drox;
use crate::ui::ThemePalette;

const FRAMES: u8 = 52;
const FRAME_MS: u64 = 50;

/// Fond splash — quasi noir, teinte verte à peine perceptible.
const SPLASH_BG: Color = Color::Rgb(0x03, 0x07, 0x04);
/// Halo radial (vert très foncé, dégradé doux).
const HALO_EDGE: Color = Color::Rgb(0x06, 0x0e, 0x07);
const HALO_MID: Color = Color::Rgb(0x0b, 0x16, 0x0c);
const HALO_CORE: Color = Color::Rgb(0x10, 0x1e, 0x11);
const TAGLINE_FG: Color = Color::Rgb(0x12, 0x22, 0x14);

/// Fin de la phase d'apparition (0..1).
const APPEAR_END: f32 = 0.48;
/// Fin du palier plein avant disparition (0..1).
const HOLD_END: f32 = 0.68;

static LOGO: &[&str] = &[
    "██████╗ ██████╗  ██████╗ ██╗  ██╗",
    "██╔══██╗██╔══██╗██╔═══██╗╚██╗██╔╝",
    "██║  ██║██████╔╝██║   ██║ ╚███╔╝ ",
    "██║  ██║██╔══██╗██║   ██║ ██╔██╗ ",
    "██████╔╝██║  ██║╚██████╔╝██╔╝ ██╗",
    "╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═╝",
];

static LOGO_COMPACT: &str = "DROX TUI";

struct Timeline {
    /// Dissolve entrant (0 → 1).
    appear: f32,
    /// Dissolve sortant (0 → 1).
    exit: f32,
    /// Intensité globale du phosphore (pic au palier).
    glow: f32,
}

/// Joue l'intro complète (~2,6 s) — toujours, sans skip clavier/souris.
pub fn play<B: ratatui::backend::Backend>(
    term: &mut Terminal<B>,
    palette: &ThemePalette,
) -> std::io::Result<()> {
    drain_buffered_events()?;

    for tick in 0..FRAMES {
        let timeline = timeline_at(tick, FRAMES);
        term.draw(|f| render(f, palette, &timeline))?;
        thread::sleep(Duration::from_millis(FRAME_MS));
    }
    drain_buffered_events()?;
    Ok(())
}

fn drain_buffered_events() -> std::io::Result<()> {
    while event::poll(Duration::from_millis(0))? {
        let _ = event::read()?;
    }
    Ok(())
}

#[must_use]
fn timeline_at(tick: u8, total: u8) -> Timeline {
    let t = tick as f32 / (total.saturating_sub(1).max(1)) as f32;

    let appear = if t < APPEAR_END {
        ease_out((t / APPEAR_END).clamp(0.0, 1.0))
    } else {
        1.0
    };

    let exit = if t > HOLD_END {
        ease_in(((t - HOLD_END) / (1.0 - HOLD_END)).clamp(0.0, 1.0))
    } else {
        0.0
    };

    let glow = if t < APPEAR_END {
        ease_out((t / APPEAR_END).clamp(0.0, 1.0)) * 0.55
    } else if t <= HOLD_END {
        0.55 + ((t - APPEAR_END) / (HOLD_END - APPEAR_END)).clamp(0.0, 1.0) * 0.25
    } else {
        0.8 * (1.0 - exit)
    };

    Timeline { appear, exit, glow }
}

#[must_use]
fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(2)
}

#[must_use]
fn ease_in(t: f32) -> f32 {
    t * t
}

#[must_use]
fn halo_strength(timeline: &Timeline) -> f32 {
    let grow = ((timeline.appear - 0.12) / 0.52).clamp(0.0, 1.0);
    grow * (1.0 - timeline.exit)
}

fn render(frame: &mut Frame, palette: &ThemePalette, timeline: &Timeline) {
    let area = frame.area();

    frame.render_widget(
        Block::default().style(Style::default().bg(SPLASH_BG)),
        area,
    );

    if area.width >= 36 && area.height >= 14 {
        let logo_rect = logo_block_rect(area);
        render_halo(frame, area, timeline, logo_rect);
        render_logo_block(frame, palette, timeline, logo_rect);
    } else {
        render_compact(frame, area, palette, timeline);
    }

    let tagline_visible = timeline.appear > 0.55 && timeline.exit < 0.75;
    if tagline_visible {
        let tagline = if timeline.appear > 0.82 && timeline.exit < 0.2 {
            i18n::t(keys::BOOT_INIT)
        } else {
            i18n::t(keys::BOOT_TAGLINE)
        };
        let y = area.height.saturating_sub(3).max(1);
        let tag_rect = Rect {
            x: area.x,
            y: area.y + y,
            width: area.width,
            height: 1,
        };
        let in_alpha = ((timeline.appear - 0.55) / 0.3).clamp(0.0, 1.0);
        let out_alpha = 1.0 - timeline.exit;
        let alpha = in_alpha * out_alpha;
        let fg = lerp_color(HALO_MID, TAGLINE_FG, alpha * 0.7);
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

#[must_use]
fn logo_block_rect(area: Rect) -> Rect {
    let logo_h = LOGO.len() as u16;
    let logo_w = LOGO.first().map(|l| l.chars().count()).unwrap_or(0) as u16;
    let x = area.x + area.width.saturating_sub(logo_w) / 2;
    let y = area.y + area.height.saturating_sub(logo_h + 4) / 2;
    Rect {
        x,
        y,
        width: logo_w.min(area.width),
        height: logo_h,
    }
}

fn render_halo(frame: &mut Frame, area: Rect, timeline: &Timeline, logo_rect: Rect) {
    let strength = halo_strength(timeline);
    if strength <= 0.02 {
        return;
    }

    let cx = logo_rect.x as f32 + logo_rect.width as f32 / 2.0;
    let cy = logo_rect.y as f32 + logo_rect.height as f32 / 2.0;
    let size_scale = 0.72 + strength * 0.38;
    let rx = (logo_rect.width as f32 * 0.78 + 8.0) * size_scale;
    let ry = (logo_rect.height as f32 * 0.92 + 5.0) * size_scale;

    let x_start = (cx - rx - 1.0).floor().max(area.x as f32) as u16;
    let x_end = (cx + rx + 1.0)
        .ceil()
        .min((area.x + area.width) as f32) as u16;
    let y_start = (cy - ry - 1.0).floor().max(area.y as f32) as u16;
    let y_end = (cy + ry + 1.0)
        .ceil()
        .min((area.y + area.height) as f32) as u16;

    if x_end <= x_start || y_end <= y_start {
        return;
    }

    for y in y_start..y_end {
        let mut spans = Vec::new();
        for x in x_start..x_end {
            let intensity = halo_intensity(x, y, cx, cy, rx, ry, strength, timeline.glow);
            if intensity < 0.05 {
                spans.push(Span::styled(" ", Style::default().bg(SPLASH_BG)));
                continue;
            }
            let ch = if intensity > 0.38 { '░' } else { '·' };
            let fg = lerp_color(HALO_EDGE, HALO_CORE, intensity * 0.6);
            spans.push(Span::styled(
                ch.to_string(),
                Style::default().fg(fg).bg(SPLASH_BG),
            ));
        }
        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                x: x_start,
                y,
                width: x_end - x_start,
                height: 1,
            },
        );
    }
}

#[must_use]
fn halo_intensity(
    x: u16,
    y: u16,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    strength: f32,
    glow: f32,
) -> f32 {
    let dx = (x as f32 + 0.5 - cx) / rx;
    let dy = (y as f32 + 0.5 - cy) / ry;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist > 1.0 {
        return 0.0;
    }
    let falloff = (1.0 - dist).powf(2.4);
    falloff * strength * (0.3 + glow * 0.7)
}

fn render_logo_block(
    frame: &mut Frame,
    palette: &ThemePalette,
    timeline: &Timeline,
    logo_rect: Rect,
) {
    let logo_in = ((timeline.appear - 0.22) / 0.5).clamp(0.0, 1.0);
    let logo_strength = logo_in * (1.0 - timeline.exit);
    if logo_strength <= 0.02 {
        return;
    }

    let peak = lerp_color(drox::PHOSPHOR_DIM, palette.accent_bright, timeline.glow * 0.65);

    let mut lines = Vec::new();
    for (row, line) in LOGO.iter().enumerate() {
        let row_reveal =
            ((logo_strength * LOGO.len() as f32) - row as f32).clamp(0.0, 1.0) * logo_strength;
        if row_reveal <= 0.0 {
            continue;
        }
        let style = if row_reveal >= 0.92 {
            Style::default()
                .fg(peak)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(lerp_color(drox::BG_ELEVATED, peak, row_reveal))
        };
        lines.push(Line::from(Span::styled(*line, style)));
    }

    let line_count = lines.len() as u16;
    frame.render_widget(
        Paragraph::new(lines),
        Rect {
            x: logo_rect.x,
            y: logo_rect.y,
            width: logo_rect.width,
            height: line_count,
        },
    );
}

fn render_compact(frame: &mut Frame, area: Rect, palette: &ThemePalette, timeline: &Timeline) {
    let center_y = area.y + area.height / 2;
    let compact_w = LOGO_COMPACT.chars().count() as u16;
    let logo_rect = Rect {
        x: area.x + area.width.saturating_sub(compact_w) / 2,
        y: center_y,
        width: compact_w.min(area.width),
        height: 1,
    };
    render_halo(frame, area, timeline, logo_rect);

    let logo_in = ((timeline.appear - 0.18) / 0.55).clamp(0.0, 1.0);
    let logo_strength = logo_in * (1.0 - timeline.exit);
    if logo_strength <= 0.02 {
        return;
    }
    let fg = lerp_color(
        drox::PHOSPHOR_DIM,
        palette.accent_bright,
        logo_strength * timeline.glow * 0.6,
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            LOGO_COMPACT,
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center),
        Rect {
            x: area.x,
            y: center_y,
            width: area.width,
            height: 1,
        },
    );
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
    fn halo_peaks_at_center() {
        let center = halo_intensity(40, 12, 40.0, 12.0, 20.0, 8.0, 1.0, 0.8);
        let edge = halo_intensity(60, 12, 40.0, 12.0, 20.0, 8.0, 1.0, 0.8);
        assert!(center > edge);
        assert!(center > 0.1);
    }

    #[test]
    fn halo_zero_outside_ellipse() {
        let far = halo_intensity(80, 30, 40.0, 12.0, 20.0, 8.0, 1.0, 0.8);
        assert!(far < 0.01);
    }

    #[test]
    fn timeline_has_hold_between_appear_and_exit() {
        let mid = timeline_at(30, FRAMES);
        assert!(mid.appear >= 0.99);
        assert!(mid.exit < 0.1);
        assert!(mid.glow > 0.5);
    }

    #[test]
    fn halo_strength_tracks_appear_and_exit() {
        let start = timeline_at(0, FRAMES);
        let peak = timeline_at(30, FRAMES);
        let fading = timeline_at(50, FRAMES);
        assert!(halo_strength(&peak) > halo_strength(&start));
        assert!(halo_strength(&peak) > halo_strength(&fading));
    }
}
