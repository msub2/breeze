#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod history;
mod networking;
mod protocols;

use std::cell::Cell;
use std::str::FromStr;

use eframe::egui;
use protocols::textprotocol::TextProtocol;
use url::Url;

use crate::history::{add_entry, can_go_back, can_go_forward};
use crate::networking::fetch;
use crate::protocols::finger::Finger;
use crate::protocols::gemini::Gemini;
use crate::protocols::gopher::Gopher;
use crate::protocols::nex::Nex;
use crate::protocols::{Protocol, ProtocolHandler};

fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Breeze",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Ok(Box::<Breeze>::new(Breeze::new()))
        }),
    )
}

#[derive(Default)]
struct ProtocolHandlers {
    finger: Finger,
    gemini: Gemini,
    gopher: Gopher,
    nex: Nex,
    textprotocol: TextProtocol,
}

struct Breeze {
    /// The current value of the URL bar
    url: Cell<String>,
    /// The last URL that was navigated to
    current_url: Url,
    /// The plaintext response from the server for this page
    page_content: String,
    protocol_handlers: ProtocolHandlers,
    navigation_hint: Cell<Option<(String, Protocol)>>,
    reset_scroll_pos: bool,
}

impl Breeze {
    fn new() -> Self {
        let starting_url = Url::from_str("gemini://geminiprotocol.net/").unwrap();
        Self {
            url: Cell::new(starting_url.to_string()),
            current_url: starting_url.clone(),
            page_content: "".to_string(),
            protocol_handlers: Default::default(),
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
            let protocol = protocol_hint.unwrap_or(Protocol::from_url(&self.current_url));
            add_entry(Url::from_str(self.url.get_mut()).unwrap(), protocol);
        }
        self.current_url = Url::from_str(self.url.get_mut()).unwrap();
        let protocol = Protocol::from_url(&self.current_url);
        if protocol == Protocol::Unknown {
            self.page_content = "Invalid URL".to_string();
            return;
        }
        match protocol {
            Protocol::Finger => {
                let mut selector = self.current_url.path();
                if selector.starts_with("/") {
                    selector = &selector[1..];
                }
                let response = fetch(
                    self.current_url.host_str().unwrap(),
                    self.current_url.port().unwrap_or(79),
                    selector,
                    false,
                );
                match response {
                    Ok(response) => {
                        self.page_content = response;
                        self.protocol_handlers
                            .finger
                            .parse_content(&self.page_content, true);
                    }
                    Err(error) => {
                        self.page_content = error;
                        self.protocol_handlers
                            .finger
                            .parse_content(&self.page_content, true);
                    }
                }
            }
            Protocol::Gemini => {
                let response = fetch(
                    self.current_url.host_str().unwrap(),
                    self.current_url.port().unwrap_or(1965),
                    &self.current_url.to_string(),
                    true,
                );
                match response {
                    Ok(response) => {
                        self.page_content = response;
                        let plaintext = protocol_hint.is_some_and(|p| p == Protocol::Plaintext);
                        self.protocol_handlers
                            .gemini
                            .parse_content(&self.page_content, plaintext);
                    }
                    Err(error) => {
                        self.page_content = error;
                        self.protocol_handlers
                            .gemini
                            .parse_content(&self.page_content, true);
                    }
                }
            }
            Protocol::Gopher => {
                let response = fetch(
                    self.current_url.host_str().unwrap(),
                    self.current_url.port().unwrap_or(70),
                    &format!(
                        "{}\t{}",
                        self.current_url.path(),
                        self.current_url.query().unwrap_or("")
                    ),
                    false,
                );
                match response {
                    Ok(response) => {
                        self.page_content = response;
                        let plaintext = protocol_hint.is_some_and(|p| p == Protocol::Plaintext);
                        self.protocol_handlers
                            .gopher
                            .parse_content(&self.page_content, plaintext);
                    }
                    Err(error) => {
                        self.page_content = error;
                        self.protocol_handlers
                            .gopher
                            .parse_content(&self.page_content, true);
                    }
                }
            }
            Protocol::Nex => {
                let response = fetch(
                    self.current_url.host_str().unwrap(),
                    self.current_url.port().unwrap_or(1900),
                    self.current_url.path(),
                    false,
                );
                match response {
                    Ok(response) => {
                        self.page_content = response;
                        self.protocol_handlers.nex.parse_content(
                            &self.page_content,
                            self.current_url.path().ends_with(".txt"),
                        );
                    }
                    Err(error) => {
                        self.page_content = error;
                        self.protocol_handlers
                            .nex
                            .parse_content(&self.page_content, true);
                    }
                }
            }
            Protocol::Spartan => {
                let response = fetch(
                    self.current_url.host_str().unwrap(),
                    self.current_url.port().unwrap_or(300),
                    &format!("{} {} {}", self.current_url.host_str().unwrap(), self.current_url.path(), 0),
                    false,
                );
                match response {
                    Ok(response) => {
                        self.page_content = response;
                        self.protocol_handlers.gemini.parse_content(
                            &self.page_content,
                            self.current_url.path().ends_with(".txt"),
                        );
                    }
                    Err(error) => {
                        self.page_content = error;
                        self.protocol_handlers
                            .gemini
                            .parse_content(&self.page_content, true);
                    }
                }
            }
            Protocol::TextProtocol => {
                let response = fetch(
                    self.current_url.host_str().unwrap(),
                    self.current_url.port().unwrap_or(1961),
                    self.current_url.as_str(),
                    false,
                );
                match response {
                    Ok(response) => {
                        self.page_content = response;
                        self.protocol_handlers.textprotocol.parse_content(
                            &self.page_content,
                            self.current_url.path().ends_with(".txt"),
                        );
                    }
                    Err(error) => {
                        self.page_content = error;
                        self.protocol_handlers
                            .textprotocol
                            .parse_content(&self.page_content, true);
                    }
                }
            }
            _ => unreachable!(),
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
            scroll_area.show(ui, |ui| match Protocol::from_url(&self.current_url) {
                Protocol::Finger => {
                    self.protocol_handlers.finger.render_page(ui, self);
                }
                Protocol::Gemini => {
                    self.protocol_handlers.gemini.render_page(ui, self);
                }
                Protocol::Gopher => {
                    self.protocol_handlers.gopher.render_page(ui, self);
                }
                Protocol::Nex => {
                    self.protocol_handlers.nex.render_page(ui, self);
                }
                Protocol::Scorpion => {
                    todo!()
                }
                Protocol::Spartan => {
                    self.protocol_handlers.gemini.render_page(ui, self);
                }
                Protocol::TextProtocol => {
                    self.protocol_handlers.textprotocol.render_page(ui, self);
                }
                Protocol::Plaintext | Protocol::Unknown => {
                    let _ = ui.monospace(&self.page_content);
                }
            });
        });

        if let Some((_, protocol)) = self.navigation_hint.take() {
            self.navigate(Some(protocol), true);
            self.reset_scroll_pos = true;
        }
    }
}
