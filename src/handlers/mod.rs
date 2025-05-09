pub mod finger;
pub mod gemtext;
pub mod gopher;
pub mod nex;
pub mod plaintext;
pub mod scorpion;

use eframe::egui;
use url::Url;

#[allow(clippy::enum_variant_names)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Protocol {
    Finger,
    Gopher(bool),
    Gemini,
    Guppy,
    Nex,
    Plaintext,
    Scorpion,
    Scroll,
    Spartan,
    TextProtocol,
    Unknown,
}

impl Protocol {
    pub fn from_url(url: &Url) -> Protocol {
        Protocol::from_str(url.scheme())
    }

    pub fn from_str(s: &str) -> Protocol {
        match s.split(':').next().unwrap() {
            "finger" => Protocol::Finger,
            "gemini" => Protocol::Gemini,
            "gopher" => Protocol::Gopher(false),
            "gophers" => Protocol::Gopher(true),
            "guppy" => Protocol::Guppy,
            "nex" => Protocol::Nex,
            "scorpion" => Protocol::Scorpion,
            "scroll" => Protocol::Scroll,
            "spartan" => Protocol::Spartan,
            "text" => Protocol::TextProtocol,
            _ => Protocol::Unknown,
        }
    }
}

pub trait ProtocolHandler {
    // Parses server text response updates internal page representation
    fn parse_content(&mut self, response: &[u8], plaintext: bool);
    fn render_page(&self, ui: &mut egui::Ui, breeze: &super::Breeze);
}
