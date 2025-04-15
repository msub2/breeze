use eframe::egui::Ui;

use crate::Breeze;

use super::ProtocolHandler;

pub struct TextProtocol {
    current_page_contents: String,
}

// Ignoring this clippy warning for now in case I decide to add link support
#[allow(clippy::derivable_impls)]
impl Default for TextProtocol {
    fn default() -> Self {
        Self {
            current_page_contents: String::new(),
        }
    }
}

impl ProtocolHandler for TextProtocol {
    fn parse_content(&mut self, response: &str, _: bool) {
        self.current_page_contents = response.to_string();
    }

    fn render_page(&self, ui: &mut Ui, _: &Breeze) {
        ui.monospace(&self.current_page_contents);
    }
}
