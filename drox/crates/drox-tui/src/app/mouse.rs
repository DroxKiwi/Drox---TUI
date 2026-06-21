//! Événements souris (scroll, clic modales).

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::app::state::{
    AiServerSelectFocus, AiServerStep, AppPhase, ConfigureField,
};
use crate::asker::parse_answer;
use crate::terminal;
use crate::ui::hit_areas::HitAreas;
use crate::widgets::{ai_server_dialog, prompt_modal};

use super::App;

impl App {
    pub(super) fn handle_mouse(&mut self, event: MouseEvent) {
        if !self.state.mouse_enabled {
            return;
        }

        match event.kind {
            MouseEventKind::ScrollUp => self.mouse_scroll(event.column, event.row, -3),
            MouseEventKind::ScrollDown => self.mouse_scroll(event.column, event.row, 3),
            MouseEventKind::Moved => self.mouse_hover(event.column, event.row),
            MouseEventKind::Down(MouseButton::Left) => {
                self.mouse_click(event.column, event.row);
            }
            _ => {}
        }
    }

    fn mouse_scroll(&mut self, x: u16, y: u16, delta: i16) {
        let areas = self.state.hit_areas;

        if self.state.scroll_viewer.is_some() {
            if areas
                .scroll_viewer_popup
                .is_some_and(|r| HitAreas::contains(r, x, y))
            {
                const PAGE: usize = 3;
                if let Some(viewer) = self.state.scroll_viewer.as_mut() {
                    if delta < 0 {
                        viewer.scroll_page(PAGE, PAGE);
                    } else {
                        viewer.scroll_page_down(PAGE, PAGE);
                    }
                }
            }
            return;
        }

        if self.state.phase == AppPhase::Prompt {
            if areas
                .prompt_body
                .is_some_and(|r| HitAreas::contains(r, x, y))
            {
                if let Some(dialog) = self.state.prompt.as_mut() {
                    if delta < 0 {
                        dialog.body_scroll = dialog.body_scroll.saturating_sub(3);
                    } else {
                        dialog.body_scroll = dialog.body_scroll.saturating_add(3);
                    }
                }
            }
            return;
        }

        if areas.message_log.width > 0
            && HitAreas::contains(areas.message_log, x, y)
            && self.state.phase != AppPhase::Running
            && !self.state.phase.is_modal()
        {
            if delta < 0 {
                self.auto_scroll = false;
                self.state.scroll = self.state.scroll.saturating_add(3);
            } else {
                self.state.scroll = self.state.scroll.saturating_sub(3);
                if self.state.scroll == 0 {
                    self.auto_scroll = true;
                }
            }
        }
    }

    fn mouse_hover(&mut self, x: u16, y: u16) {
        let log = self.state.hit_areas.message_log;
        if HitAreas::contains(log, x, y) && log.height > 2 {
            let row = y.saturating_sub(log.y + 1) as usize;
            let max = log.height.saturating_sub(2) as usize;
            self.state.hover_log_row = Some(row.min(max.saturating_sub(1)));
        } else {
            self.state.hover_log_row = None;
        }
    }

    fn mouse_click(&mut self, x: u16, y: u16) {
        match self.state.phase {
            AppPhase::Prompt => self.mouse_click_prompt(x, y),
            AppPhase::AiServer => self.mouse_click_ai_server(x, y),
            _ => {}
        }
    }

    fn mouse_click_prompt(&mut self, x: u16, y: u16) {
        let Some(footer) = self.state.hit_areas.prompt_footer else {
            return;
        };
        if !HitAreas::contains(footer, x, y) {
            return;
        }
        let Some(dialog) = self.state.prompt.as_ref() else {
            return;
        };
        let row = y.saturating_sub(footer.y);
        let Some(choice_idx) = prompt_modal::footer_choice_at(dialog, row) else {
            return;
        };
        let question = dialog.question.clone();
        let answer = parse_answer(&question, (choice_idx + 1).to_string());
        self.complete_prompt(answer);
    }

    fn mouse_click_ai_server(&mut self, x: u16, y: u16) {
        if self
            .state
            .ai_server
            .as_ref()
            .is_some_and(|d| d.step == AiServerStep::Testing)
        {
            return;
        }

        let area = self.state.hit_areas.terminal;
        let inner = ai_server_dialog::popup_inner(area);
        if !HitAreas::contains(inner, x, y) {
            return;
        }

        let line = y.saturating_sub(inner.y) as usize;
        let hit = self
            .state
            .ai_server
            .as_ref()
            .and_then(|d| ai_server_dialog::hit_at_line(d, line));

        let Some(hit) = hit else {
            return;
        };

        match hit {
            ai_server_dialog::AiServerHit::ListItem(i) => {
                if let Some(dialog) = self.state.ai_server.as_mut() {
                    dialog.list_cursor = i;
                    dialog.wizard_advance_list();
                }
            }
            ai_server_dialog::AiServerHit::TestButton => {
                if let Some(d) = self.state.ai_server.as_mut() {
                    d.configure_focus = ConfigureField::TestButton;
                }
                self.start_ai_server_test();
            }
            ai_server_dialog::AiServerHit::BackButton => {
                if let Some(dialog) = self.state.ai_server.as_mut() {
                    dialog.wizard_back();
                }
            }
            ai_server_dialog::AiServerHit::AddHeader => {
                if let Some(dialog) = self.state.ai_server.as_mut() {
                    dialog.configure_focus = ConfigureField::AddExtraHeader;
                    dialog.add_extra_header_from_inputs();
                }
            }
            ai_server_dialog::AiServerHit::ConfirmReset => {
                self.state.ai_server = Some(crate::app::state::AiServerDialog::new_wizard());
                self.state.status_line =
                    "/server — nouvelle configuration (Ctrl+Shift+L)".into();
            }
            ai_server_dialog::AiServerHit::CancelReset => {
                if let Some(dialog) = self.state.ai_server.as_mut() {
                    dialog.wizard_back();
                }
            }
            ai_server_dialog::AiServerHit::Model(i) => {
                if let Some(dialog) = self.state.ai_server.as_mut() {
                    dialog.model_cursor = i;
                    dialog.select_focus = AiServerSelectFocus::ModelList;
                }
                let _ = self.handle_ai_server_key(crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Enter,
                    crossterm::event::KeyModifiers::NONE,
                ));
            }
            ai_server_dialog::AiServerHit::ResetWizard => {
                if let Some(dialog) = self.state.ai_server.as_mut() {
                    dialog.select_focus = AiServerSelectFocus::ResetWizard;
                    dialog.begin_reset_wizard();
                }
            }
        }
    }

    pub(super) fn apply_mouse_setting(&mut self, enabled: bool) {
        self.state.mouse_enabled = enabled;
        self.tui_prefs.mouse_enabled = enabled;
        let mut prefs = crate::engine::preferences::load_preferences();
        prefs.mouse_enabled = enabled;
        let _ = crate::engine::preferences::save_preferences(&prefs);
        let _ = terminal::set_mouse_capture(enabled);
        self.state.status_line = if enabled {
            crate::i18n::t(crate::i18n::keys_p1::STATUS_MOUSE_ON).into()
        } else {
            crate::i18n::t(crate::i18n::keys_p1::STATUS_MOUSE_OFF).into()
        };
    }
}
