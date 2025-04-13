pub mod finger;
pub mod gopher;

use eframe::egui;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Protocol {
    Finger,
    Gopher,
    Gemini,
    Plaintext,
    Scorpion,
    Unknown,
}

pub trait ProtocolHandler {
    // Parses server text response updates internal page representation
    fn parse_content(&mut self, response: &str, plaintext: bool);
    fn render_page(&self, ui: &mut egui::Ui, breeze: &super::Breeze);
}
