pub mod finger;
pub mod gemini;
pub mod gopher;
pub mod nex;
pub mod textprotocol;

use eframe::egui;
use url::Url;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Protocol {
    Finger,
    Gopher,
    Gemini,
    Nex,
    Plaintext,
    Scorpion,
    TextProtocol,
    Unknown,
}

impl Protocol {
    pub fn from_url(url: &Url) -> Protocol {
        match url.scheme() {
            "finger" => Protocol::Finger,
            "gemini" => Protocol::Gemini,
            "gopher" => Protocol::Gopher,
            "nex" => Protocol::Nex,
            "scorpion" => Protocol::Scorpion,
            "text" => Protocol::TextProtocol,
            _ => Protocol::Unknown,
        }
    }
}

pub trait ProtocolHandler {
    // Parses server text response updates internal page representation
    fn parse_content(&mut self, response: &str, plaintext: bool);
    fn render_page(&self, ui: &mut egui::Ui, breeze: &super::Breeze);
}
