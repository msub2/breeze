use eframe::egui::{RichText, Ui};

use crate::Breeze;

use super::ProtocolHandler;

#[derive(Default)]
pub struct Plaintext {
    current_page_contents: String,
}

impl ProtocolHandler for Plaintext {
    fn parse_content(&mut self, response: &str, _: bool) {
        self.current_page_contents = response.to_string();
    }

    fn render_page(&self, ui: &mut Ui, _: &Breeze) {
        let text = RichText::new(&self.current_page_contents).size(14.0);
        ui.monospace(text);
    }
}
