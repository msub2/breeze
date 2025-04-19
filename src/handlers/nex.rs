use eframe::egui::{self, Color32, Label, RichText, Ui};

use crate::{Breeze, NavigationHint};

use super::{Protocol, ProtocolHandler};

struct NexLine {
    text: String,
    is_link: bool,
}

impl NexLine {
    fn from_str(s: &str) -> Self {
        Self {
            text: s.to_string(),
            is_link: s.starts_with("=> "),
        }
    }
}

#[derive(Default)]
pub struct Nex {
    current_page_contents: Vec<NexLine>,
}

impl ProtocolHandler for Nex {
    fn parse_content(&mut self, response: &[u8], plaintext: bool) {
        let response = String::from_utf8_lossy(response);
        if plaintext {
            self.current_page_contents = vec![NexLine::from_str(&response)];
        } else {
            self.current_page_contents = response.lines().map(NexLine::from_str).collect();
        }
    }

    fn render_page(&self, ui: &mut Ui, breeze: &Breeze) {
        for line in &self.current_page_contents {
            if line.is_link {
                ui.horizontal(|ui| {
                    let (label, url) = line.text.split_once(' ').unwrap();
                    ui.label(label);
                    let link_text = RichText::new(url)
                        .color(Color32::BLUE)
                        .underline()
                        .monospace();
                    let link = ui.add(Label::new(link_text).sense(egui::Sense::hover()));
                    if link.hovered() {
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                    }
                    if link.clicked() {
                        let current_url = breeze.current_url.join(url).unwrap();
                        let url = current_url.to_string();
                        breeze.url.set(url.clone());
                        breeze.navigation_hint.set(Some(NavigationHint {
                            url,
                            protocol: Protocol::Nex,
                            add_to_history: true,
                        }));
                    }
                });
            } else {
                ui.monospace(&line.text);
            }
        }
    }
}
