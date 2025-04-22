use std::cell::Cell;

use eframe::egui::{self, Color32, Label, RichText, TextEdit, Ui, Vec2};

use crate::{Breeze, NavigationHint};

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
    // Spartan Additions
    Prompt,
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
        } else if s.starts_with("=:") {
            LineType::Prompt
        } else {
            LineType::Text
        }
    }
}

struct GemtextLine {
    line_type: LineType,
    content: String,
    path: Option<String>,
    preformatted: bool,
    prompt_string: Cell<String>,
}

impl GemtextLine {
    fn from_str(s: &str, plaintext: bool, gemtext: &mut Gemtext) -> Self {
        if plaintext || s.is_empty() {
            // Treat every line as an informational one
            return Self {
                line_type: LineType::Text,
                content: s.to_string(),
                path: None,
                preformatted: gemtext.preformat_line,
                prompt_string: Cell::new("".to_string()),
            };
        }

        let line_type = LineType::from_str(s);
        if line_type == LineType::PreformatToggle {
            gemtext.preformat_line = !gemtext.preformat_line;
        }
        let (content, path) = if gemtext.preformat_line && line_type != LineType::PreformatToggle {
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
                LineType::Prompt => {
                    let content = s[2..].trim().to_string();
                    match content.split_once(char::is_whitespace) {
                        Some((path, display_string)) => {
                            (display_string.trim().to_string(), Some(path.to_string()))
                        }
                        None => (content.clone(), Some(content)),
                    }
                }
                _ => (s.to_string(), None),
            }
        };

        Self {
            line_type,
            content,
            path,
            preformatted: gemtext.preformat_line,
            prompt_string: Cell::new("".to_string()),
        }
    }
}

#[derive(Default)]
pub struct Gemtext {
    current_page_contents: Vec<GemtextLine>,
    preformat_line: bool,
}

impl ProtocolHandler for Gemtext {
    fn parse_content(&mut self, response: &[u8], plaintext: bool) {
        let response = String::from_utf8_lossy(response);
        self.preformat_line = false; // Reset preformat flag on new page load
        if plaintext {
            let lines: Vec<&str> = response.lines().filter(|line| line != &".").collect();
            let gemtext_line = GemtextLine::from_str(&lines.join("\n"), plaintext, self);
            self.current_page_contents = vec![gemtext_line];
            return;
        }
        self.current_page_contents = response
            .lines()
            .filter_map(|line| {
                if line == "." {
                    // EOF
                    None
                } else {
                    Some(GemtextLine::from_str(line, plaintext, self))
                }
            })
            .collect();
    }

    fn render_page(&self, ui: &mut Ui, breeze: &Breeze) {
        ui.style_mut().spacing.item_spacing = Vec2::new(0.0, -1.0);
        for line in &self.current_page_contents {
            ui.horizontal(|ui| {
                if line.preformatted && line.line_type != LineType::PreformatToggle {
                    let mut padded_text = line.content.clone();
                    let padding_needed = 120_usize.saturating_sub(padded_text.len());
                    padded_text.push_str(&" ".repeat(padding_needed));
                    let text = RichText::new(&padded_text).code().size(14.0);
                    ui.add_sized([120.0, 16.0], egui::Label::new(text).extend());
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
                            let path = line.path.clone().expect("Gemtext link line without path!");
                            let current_url = breeze.current_url.clone();
                            let current_url = current_url.join(&path).unwrap();

                            let link = ui.add(Label::new(link_text).sense(egui::Sense::hover()));
                            if link.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                *breeze.status_text.borrow_mut() = current_url.to_string();
                            }
                            if link.clicked() {
                                breeze.url.set(current_url.to_string());
                                let hint = if path.ends_with(".txt") {
                                    Protocol::Plaintext
                                } else {
                                    Protocol::from_url(&current_url)
                                };
                                breeze.navigation_hint.set(Some(NavigationHint {
                                    url: current_url.to_string(),
                                    protocol: hint,
                                    add_to_history: true,
                                }));
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
                        LineType::Prompt => {
                            let mut current_prompt = line.prompt_string.take();
                            ui.add(TextEdit::singleline(&mut current_prompt));
                            line.prompt_string.replace(current_prompt.clone());
                            if ui.button("Submit").clicked() {
                                let path =
                                    line.path.clone().expect("Gemtext link line without path!");
                                let current_url = breeze.current_url.clone();
                                let mut current_url = current_url.join(&path).unwrap();
                                current_url.set_query(Some(&current_prompt));
                                breeze.url.set(current_url.to_string());
                                let hint = if path.ends_with(".txt") {
                                    Protocol::Plaintext
                                } else {
                                    Protocol::from_url(&current_url)
                                };
                                breeze.navigation_hint.set(Some(NavigationHint {
                                    url: current_url.to_string(),
                                    protocol: hint,
                                    add_to_history: true,
                                }));
                            }
                        }
                    }
                }
            });
        }
    }
}
