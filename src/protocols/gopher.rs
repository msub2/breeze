use std::cell::Cell;

use eframe::egui::{self, Color32, Label, RichText, TextEdit, Ui};

use crate::Breeze;

use super::{Protocol, ProtocolHandler};

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
enum LineType {
    Text,
    Submenu,
    CCSONameserver,
    Error,
    BinHexFile,
    DOSFile,
    UUencodedFile,
    Search,
    Telnet,
    BinaryFile,
    Mirror,
    GIFFile,
    ImageFile,
    Telnet3270,
    // Gopher+
    BitmapImage,
    MovieFile,
    SoundFile, // > or s
    // Non Canonical
    Document,
    HTML,
    Informational,
    PNGFile,
    RTFFile,
    PDFFile,
    XMLFile,
    Unknown,
}

impl LineType {
    fn from_str(s: &str) -> LineType {
        match s {
            "0" => LineType::Text,
            "1" => LineType::Submenu,
            "2" => LineType::CCSONameserver,
            "3" => LineType::Error,
            "4" => LineType::BinHexFile,
            "5" => LineType::DOSFile,
            "6" => LineType::UUencodedFile,
            "7" => LineType::Search,
            "8" => LineType::Telnet,
            "9" => LineType::BinaryFile,
            "+" => LineType::Mirror,
            "g" => LineType::GIFFile,
            "I" => LineType::ImageFile,
            "T" => LineType::Telnet3270,
            ":" => LineType::BitmapImage,
            ";" => LineType::MovieFile,
            "<" => LineType::SoundFile,
            "d" => LineType::Document,
            "h" => LineType::HTML,
            "i" => LineType::Informational,
            "p" => LineType::PNGFile,
            "r" => LineType::RTFFile,
            "s" => LineType::SoundFile,
            "t" => LineType::PDFFile,
            "x" => LineType::XMLFile,
            _ => LineType::Unknown,
        }
    }

    fn icon(&self) -> &str {
        match self {
            LineType::Text => "🖹",
            LineType::Submenu => "🗁",
            LineType::CCSONameserver => "📞",
            LineType::Error => "⚠",
            LineType::Search => "🔍",
            LineType::HTML => "🌐",
            _ => " ",
        }
    }
}

struct GopherLine {
    line_type: LineType,
    user_display_string: String,
    selector: String,
    hostname: String,
    port: u16,
    is_link: bool,
    search_string: Cell<String>,
}

impl GopherLine {
    fn from_str(s: &str, plaintext: bool) -> Self {
        if plaintext {
            // Treat every line as an informational one
            return Self {
                line_type: LineType::Informational,
                user_display_string: s.to_string(),
                selector: "".to_string(),
                hostname: "".to_string(),
                port: 0,
                is_link: false,
                search_string: Cell::new("".to_string()),
            };
        }
        let (line_type, content) = match s.split_at_checked(1) {
            Some((line_type, content)) => (line_type, content),
            None => ("i", s),
        };
        let components = content.split("\t").collect::<Vec<&str>>();
        if components.len() == 1 {
            // EOF, just insert a blank line
            return Self {
                line_type: LineType::Informational,
                user_display_string: "".to_string(),
                selector: "".to_string(),
                hostname: "".to_string(),
                port: 0,
                is_link: false,
                search_string: Cell::new("".to_string()),
            };
        }

        let user_display_string = components[0].to_string();
        let selector = components[1].to_string();
        let hostname = components[2].to_string();
        let port = components[3].parse().expect("Invalid port number!");
        let is_link = !matches!(line_type, "i" | "7");

        Self {
            line_type: LineType::from_str(line_type),
            user_display_string,
            selector,
            hostname,
            port,
            is_link,
            search_string: Cell::new("".to_string()),
        }
    }
}

#[derive(Default)]
pub struct Gopher {
    current_page_contents: Vec<GopherLine>,
}

impl ProtocolHandler for Gopher {
    fn parse_content(&mut self, response: &str, plaintext: bool) {
        if plaintext {
            let lines: Vec<&str> = response.lines().filter(|line| line != &".").collect();
            let gopher_line = GopherLine::from_str(&lines.join("\n"), plaintext);
            self.current_page_contents = vec![gopher_line];
            return;
        }
        self.current_page_contents = response
            .lines()
            .filter_map(|line| {
                if line == "." {
                    // EOF
                    None
                } else {
                    Some(GopherLine::from_str(line, plaintext))
                }
            })
            .collect();
    }

    fn render_page(&self, ui: &mut Ui, breeze: &Breeze) {
        for line in &self.current_page_contents {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [16.0, 16.0],
                    Label::new(RichText::new(line.line_type.icon()).monospace()),
                );
                if line.line_type == LineType::Search {
                    let mut current_search = line.search_string.take();
                    ui.add(TextEdit::singleline(&mut current_search).hint_text("Search"));
                    line.search_string.replace(current_search.clone());
                    if ui.button("Search").clicked() {
                        let port = if line.port != 70 {
                            format!(":{}", line.port)
                        } else {
                            "".to_string()
                        };
                        let url = format!(
                            "gopher://{}{}{}?{}",
                            line.hostname, port, line.selector, &current_search
                        );
                        breeze.url.set(url.clone());
                        breeze.navigation_hint.set(Some((url, Protocol::Gopher)));
                    }
                } else if line.is_link {
                    let link_text = RichText::new(&line.user_display_string)
                        .color(Color32::BLUE)
                        .underline()
                        .monospace();
                    let link = ui.add(Label::new(link_text).sense(egui::Sense::hover()));
                    if link.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if link.clicked() {
                        let port = if line.port != 70 {
                            format!(":{}", line.port)
                        } else {
                            "".to_string()
                        };
                        let url = format!("gopher://{}{}{}", line.hostname, port, line.selector);
                        breeze.url.set(url.clone());
                        let hint = if line.line_type == LineType::Text {
                            Protocol::Plaintext
                        } else {
                            Protocol::Gopher
                        };
                        breeze.navigation_hint.set(Some((url, hint)));
                    }
                } else {
                    ui.monospace(&line.user_display_string);
                }
            });
        }
    }
}
