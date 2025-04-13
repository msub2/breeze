use eframe::egui::{self, Color32, Label, RichText, Ui};

use crate::Breeze;

use super::{Protocol, ProtocolHandler};

pub struct Finger {
    current_page_contents: String,
}

impl Default for Finger {
    fn default() -> Self {
        Self {
            current_page_contents: String::new(),
        }
    }
}

impl ProtocolHandler for Finger {
    fn parse_content(&mut self, response: &str, _: bool) {
        self.current_page_contents = response.to_string();
    }

    fn render_page(&self, ui: &mut Ui, _: &Breeze) {
        ui.monospace(&self.current_page_contents);
    }
}
