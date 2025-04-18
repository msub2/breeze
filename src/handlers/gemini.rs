use eframe::egui::{self, Color32, Label, RichText, Ui, Vec2};

use crate::Breeze;

use super::{Protocol, ProtocolHandler};

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
enum LineType {
    Text,
    Link,
    Heading1,
    Heading2,
    Heading3,
    List,
    Quote,
    PreformatToggle,
}

impl LineType {
    fn from_str(s: &str) -> LineType {
        if s.starts_with("=>") {
            LineType::Link
        } else if s.starts_with("###") {
            LineType::Heading3
        } else if s.starts_with("##") {
            LineType::Heading2
        } else if s.starts_with("#") {
            LineType::Heading1
        } else if s.starts_with(">") {
            LineType::Quote
        } else if s.starts_with("```") {
            LineType::PreformatToggle
        } else if s.starts_with("*") {
            LineType::List
        } else {
            LineType::Text
        }
    }
}

struct GeminiLine {
    line_type: LineType,
    content: String,
    path: Option<String>,
    preformatted: bool,
}

impl GeminiLine {
    fn from_str(s: &str, plaintext: bool, gemini: &mut Gemini) -> Self {
        if plaintext || s.is_empty() {
            // Treat every line as an informational one
            return Self {
                line_type: LineType::Text,
                content: s.to_string(),
                path: None,
                preformatted: gemini.preformat_line,
            };
        }

        let line_type = LineType::from_str(s);
        if line_type == LineType::PreformatToggle {
            gemini.preformat_line = !gemini.preformat_line;
        }
        let (content, path) = if gemini.preformat_line && line_type != LineType::PreformatToggle {
            (s.to_string(), None)
        } else {
            match line_type {
                LineType::Link => {
                    let content = s[2..].trim().to_string();
                    match content.split_once(char::is_whitespace) {
                        Some((path, display_string)) => {
                            (display_string.trim().to_string(), Some(path.to_string()))
                        }
                        None => (content.clone(), Some(content)),
                    }
                }
                LineType::Text => (s.to_string(), None),
                LineType::PreformatToggle => ("".to_string(), None),
                _ => (s.to_string(), None),
            }
        };

        Self {
            line_type,
            content,
            path,
            preformatted: gemini.preformat_line,
        }
    }
}

#[derive(Default)]
pub struct Gemini {
    current_page_contents: Vec<GeminiLine>,
    preformat_line: bool,
}

impl ProtocolHandler for Gemini {
    fn parse_content(&mut self, response: &str, plaintext: bool) {
        self.preformat_line = false; // Reset preformat flag on new page load
        let Some((_server_code, response)) = response.split_once("\n") else {
            return;
        };
        if plaintext {
            let lines: Vec<&str> = response.lines().filter(|line| line != &".").collect();
            let gemini_line = GeminiLine::from_str(&lines.join("\n"), plaintext, self);
            self.current_page_contents = vec![gemini_line];
            return;
        }
        self.current_page_contents = response
            .lines()
            .filter_map(|line| {
                if line == "." {
                    // EOF
                    None
                } else {
                    Some(GeminiLine::from_str(line, plaintext, self))
                }
            })
            .collect();
    }

    fn render_page(&self, ui: &mut Ui, breeze: &Breeze) {
        ui.style_mut().spacing.item_spacing = Vec2::new(0.0, -2.0);
        for line in &self.current_page_contents {
            ui.horizontal(|ui| {
                if line.preformatted && line.line_type != LineType::PreformatToggle {
                    let mut padded_text = line.content.clone();
                    let padding_needed = 80_usize.saturating_sub(padded_text.len());
                    padded_text.push_str(&" ".repeat(padding_needed));
                    let text = RichText::new(&padded_text)
                        .background_color(Color32::LIGHT_GRAY)
                        .monospace()
                        .size(14.0);
                    ui.add_sized([80.0, 16.0], egui::Label::new(text).extend());
                } else {
                    match line.line_type {
                        LineType::Text => {
                            let text = RichText::new(&line.content).size(14.0);
                            let label = egui::Label::new(text).wrap_mode(egui::TextWrapMode::Wrap);
                            ui.add(label);
                        }
                        LineType::Heading1 => {
                            let content = line.content.replace("# ", "");
                            ui.label(RichText::new(&content).size(24.0));
                        }
                        LineType::Heading2 => {
                            let content = line.content.replace("## ", "");
                            ui.label(RichText::new(&content).size(22.0));
                        }
                        LineType::Heading3 => {
                            let content = line.content.replace("### ", "");
                            ui.label(RichText::new(&content).size(20.0));
                        }
                        LineType::Link => {
                            let link_text = RichText::new(&line.content)
                                .color(Color32::BLUE)
                                .underline()
                                .size(14.0);
                            let link = ui.add(Label::new(link_text).sense(egui::Sense::hover()));
                            if link.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if link.clicked() {
                                let path =
                                    line.path.clone().expect("Gemini link line without path!");
                                let current_url = breeze.current_url.clone();
                                let current_url = current_url.join(&path).unwrap();
                                breeze.url.set(current_url.to_string());
                                let hint = if path.ends_with(".txt") {
                                    Protocol::Plaintext
                                } else {
                                    Protocol::Gemini
                                };
                                breeze
                                    .navigation_hint
                                    .set(Some((current_url.to_string(), hint)));
                            }
                        }
                        LineType::Quote => {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("| ").size(14.0));
                                ui.label(RichText::new(&line.content).italics().size(14.0))
                            });
                        }
                        LineType::List => {
                            let content = line.content.replace("*", "•");
                            ui.label(RichText::new(content).size(14.0));
                        }
                        LineType::PreformatToggle => {}
                    }
                }
            });
        }
    }
}
