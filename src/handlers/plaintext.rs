use eframe::egui::{RichText, Ui};

use crate::Breeze;

use super::ProtocolHandler;

#[derive(Default)]
pub struct Plaintext {
    current_page_contents: String,
}

impl ProtocolHandler for Plaintext {
    fn parse_content(&mut self, response: &[u8], _: bool) {
        let response = String::from_utf8_lossy(response);
        self.current_page_contents = response.to_string();
    }

    fn render_page(&self, ui: &mut Ui, _: &Breeze) {
        let text = RichText::new(&self.current_page_contents).size(14.0);
        ui.monospace(text);
    }
}
