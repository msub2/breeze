#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod history;
mod networking;
mod protocols;

use std::cell::Cell;
use std::str::FromStr;

use eframe::egui::{self};
use egui::{FontData, FontDefinitions, FontFamily};
use url::Url;

use crate::history::{add_entry, can_go_back, can_go_forward};
use crate::networking::{fetch, fetch_udp};
use crate::protocols::finger::Finger;
use crate::protocols::gemini::Gemini;
use crate::protocols::gopher::Gopher;
use crate::protocols::nex::Nex;
use crate::protocols::plaintext::Plaintext;
use crate::protocols::{Protocol, ProtocolHandler};

fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    // Set up custom fonts needed for rendering
    let mut fonts = FontDefinitions::default();
    // Inconsolata for uniform monospace font
    fonts.font_data.insert(
        "Inconsolata".to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../res/Inconsolata.ttf"
        ))),
    );
    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .insert(0, "Inconsolata".to_owned());
    // Segoe UI Symbols for rendering extended Unicode chars
    fonts.font_data.insert(
        "SegoeUISymbol".to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../res/segoe-ui-symbol.ttf"
        ))),
    );
    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .push("SegoeUISymbol".to_string());

    eframe::run_native(
        "Breeze",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::<Breeze>::new(Breeze::new()))
        }),
    )
}

#[derive(Default)]
struct ContentHandlers {
    finger: Finger,
    gemtext: Gemini,
    gopher: Gopher,
    nex: Nex,
    plaintext: Plaintext,
}

impl ContentHandlers {
    pub fn parse_content(&mut self, response: &str, plaintext: bool, protocol: Protocol) {
        match protocol {
            Protocol::Finger => self.finger.parse_content(response, plaintext),
            Protocol::Gemini | Protocol::Spartan | Protocol::Guppy => {
                self.gemtext.parse_content(response, plaintext)
            }
            Protocol::Gopher => self.gopher.parse_content(response, plaintext),
            Protocol::Nex => self.nex.parse_content(response, plaintext),
            _ => self.plaintext.parse_content(response, plaintext),
        }
    }
}

struct Breeze {
    /// The current value of the URL bar
    url: Cell<String>,
    /// The last URL that was navigated to
    current_url: Url,
    /// The plaintext response from the server for this page
    page_content: String,
    content_handlers: ContentHandlers,
    navigation_hint: Cell<Option<(String, Protocol)>>,
    reset_scroll_pos: bool,
}

impl Breeze {
    fn new() -> Self {
        let starting_url = Url::from_str("guppy://gemini.dimakrasner.com/").unwrap();
        Self {
            url: Cell::new(starting_url.to_string()),
            current_url: starting_url.clone(),
            page_content: "".to_string(),
            content_handlers: Default::default(),
            navigation_hint: Cell::new(Some((
                starting_url.to_string(),
                Protocol::from_url(&starting_url),
            ))),
            reset_scroll_pos: false,
        }
    }

    // Validate URL before updating the currently active page content
    fn navigate(&mut self, protocol_hint: Option<Protocol>, should_add_entry: bool) {
        if should_add_entry {
            println!("{}", self.url.get_mut());
            let protocol = protocol_hint.unwrap_or(Protocol::from_url(&self.current_url));
            add_entry(Url::from_str(self.url.get_mut()).unwrap(), protocol);
        }
        self.current_url = Url::from_str(self.url.get_mut()).unwrap();
        let protocol = Protocol::from_url(&self.current_url);
        if protocol == Protocol::Unknown {
            self.page_content = "Invalid URL".to_string();
            return;
        }

        let hostname = self.current_url.host_str().expect("Hostname is empty!");
        let path = self.current_url.path();
        let plaintext = protocol_hint.is_some_and(|p| p == Protocol::Plaintext);
        let response = match protocol {
            Protocol::Finger => {
                let port = self.current_url.port().unwrap_or(79);
                let selector = if let Some(stripped) = path.strip_prefix("/") {
                    stripped
                } else {
                    path
                };
                fetch(hostname, port, selector, false)
            }
            Protocol::Gemini => {
                let port = self.current_url.port().unwrap_or(1965);
                fetch(hostname, port, self.current_url.as_str(), true)
            }
            Protocol::Gopher => {
                let port = self.current_url.port().unwrap_or(70);
                let selector = &format!("{}\t{}", path, self.current_url.query().unwrap_or(""));
                fetch(hostname, port, selector, false)
            }
            Protocol::Guppy => {
                let port = self.current_url.port().unwrap_or(6775);
                fetch_udp(hostname, port, self.current_url.as_str(), false)
            }
            Protocol::Nex => {
                let port = self.current_url.port().unwrap_or(1900);
                fetch(hostname, port, path, false)
            }
            Protocol::Scorpion => {
                let port = self.current_url.port().unwrap_or(1517);
                fetch(hostname, port, self.current_url.as_str(), false)
            }
            Protocol::Spartan => {
                let port = self.current_url.port().unwrap_or(300);
                let selector = &format!("{} {} {}", hostname, path, 0);
                fetch(hostname, port, selector, false)
            }
            Protocol::TextProtocol => {
                let port = self.current_url.port().unwrap_or(1961);
                let selector = self.current_url.as_str();
                fetch(hostname, port, selector, false)
            }
            _ => unreachable!(),
        };
        match response {
            Ok(response) => {
                println!("{}", response);
                self.content_handlers
                    .parse_content(&response, plaintext, protocol);
            }
            Err(error) => {
                self.content_handlers.parse_content(&error, true, protocol);
            }
        }
    }
}

impl eframe::App for Breeze {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Navigation and address bar
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(can_go_back(), egui::Button::new("Back"))
                    .clicked()
                {
                    if let Some(entry) = history::back() {
                        self.url.set(entry.url.to_string());
                        self.navigate(Some(entry.protocol), false);
                    }
                }
                if ui
                    .add_enabled(can_go_forward(), egui::Button::new("Forward"))
                    .clicked()
                {
                    if let Some(entry) = history::forward() {
                        self.url.set(entry.url.to_string());
                        self.navigate(Some(entry.protocol), false);
                    }
                }
                let url = ui.text_edit_singleline(self.url.get_mut());
                if url.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    self.navigate(None, true);
                }
                if ui.button("Go").clicked() {
                    self.navigate(None, true);
                }
            });
            ui.separator();

            // Page content
            let mut scroll_area = egui::ScrollArea::both().auto_shrink(false);
            if self.reset_scroll_pos {
                scroll_area = scroll_area.scroll_offset([0.0, 0.0].into());
                self.reset_scroll_pos = false;
            }
            scroll_area.show(ui, |ui| {
                let protocol = Protocol::from_url(&self.current_url);
                match protocol {
                    Protocol::Finger => {
                        self.content_handlers.finger.render_page(ui, self);
                    }
                    Protocol::Gemini | Protocol::Spartan | Protocol::Guppy => {
                        self.content_handlers.gemtext.render_page(ui, self);
                    }
                    Protocol::Gopher => {
                        self.content_handlers.gopher.render_page(ui, self);
                    }
                    _ => {
                        self.content_handlers.plaintext.render_page(ui, self);
                    }
                }
            });
        });

        if let Some((_, protocol)) = self.navigation_hint.take() {
            self.navigate(Some(protocol), true);
            self.reset_scroll_pos = true;
        }
    }
}
