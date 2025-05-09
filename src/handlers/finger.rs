use eframe::egui::{RichText, Ui};

use crate::Breeze;

use super::ProtocolHandler;

pub struct Finger {
    current_page_contents: String,
}

// Ignoring this clippy warning for now in case I decide to add link support
#[allow(clippy::derivable_impls)]
impl Default for Finger {
    fn default() -> Self {
        Self {
            current_page_contents: String::new(),
        }
    }
}

impl ProtocolHandler for Finger {
    fn parse_content(&mut self, response: &[u8], _: bool) {
        let response = String::from_utf8_lossy(response);
        self.current_page_contents = response.to_string();
    }

    fn render_page(&self, ui: &mut Ui, _: &Breeze) {
        let text = RichText::new(&self.current_page_contents).size(14.0);
        ui.monospace(text);
    }
}
