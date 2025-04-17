#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod history;
mod networking;
mod protocols;

use std::cell::Cell;
use std::str::FromStr;
use std::sync::Arc;

use eframe::egui::{
    Button, CentralPanel, Context, FontData, FontDefinitions, FontFamily, IconData, Key,
    ScrollArea, ViewportBuilder,
};
use url::Url;

use crate::history::{add_entry, can_go_back, can_go_forward};
use crate::networking::fetch;
use crate::protocols::finger::Finger;
use crate::protocols::gemini::Gemini;
use crate::protocols::gopher::Gopher;
use crate::protocols::nex::Nex;
use crate::protocols::plaintext::Plaintext;
use crate::protocols::{Protocol, ProtocolHandler};

fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let icon = include_bytes!("../res/breeze32.png");
    let image = image::load_from_memory(icon)
        .expect("Failed to open icon path")
        .to_rgba8();
    let viewport = ViewportBuilder::default()
        .with_inner_size([640.0, 480.0])
        .with_icon(IconData {
            rgba: image.into_raw(),
            width: 32,
            height: 32,
        });
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    // Set up custom fonts needed for rendering
    let mut fonts = FontDefinitions::default();
    // Inconsolata for uniform monospace font
    load_font!(fonts, FontFamily::Monospace, "Inconsolata".to_string(), "../res/Inconsolata.ttf");
    // Segoe UI Symbols for rendering extended Unicode chars
    load_font!(fonts, FontFamily::Monospace, "SegoeUISymbol".to_string(), "../res/SegoeUISymbol.ttf");
    // Yu Gothic for rendering more extended chars in gemtext
    load_font!(fonts, FontFamily::Proportional, "YuGothic".to_string(), "../res/YuGothic.ttf");

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

#[macro_export]
macro_rules! load_font {
    ($fonts:expr, $font_family:expr, $font_name:expr, $font_path:expr) => {
        $fonts.font_data.insert(
            $font_name.clone(),
            Arc::new(FontData::from_static(include_bytes!($font_path)))
        );
        
        $fonts
            .families
            .get_mut(&$font_family)
            .unwrap()
            .push($font_name);
    };
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
            Protocol::Gopher(_) => self.gopher.parse_content(response, plaintext),
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
        let starting_url = Url::from_str("scorpion://zzo38computer.org/").unwrap();
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

        let current_url = self.current_url.to_string();
        let hostname = self.current_url.host_str().expect("Hostname is empty!");
        let mut path = self.current_url.path().to_string();
        if path.is_empty() {
            path = "/".to_string();
        }
        let query = if let Some(q) = self.current_url.query() {
            &format!("\t{}", q)
        } else {
            ""
        };
        let plaintext = protocol_hint.is_some_and(|p| p == Protocol::Plaintext);
        let (selector, ssl) = match protocol {
            Protocol::Finger => (path.strip_prefix("/").unwrap_or(&path).to_string(), false),
            Protocol::Gemini => (current_url, true),
            Protocol::Gopher(ssl) => (format!("{}{}", path, query), ssl),
            Protocol::Guppy => (current_url, false),
            Protocol::Nex => (path, false),
            Protocol::Scorpion => (format!("R {}", current_url), false),
            Protocol::Spartan => (format!("{} {} {}", hostname, path, 0), false),
            Protocol::TextProtocol => (current_url, false),
            _ => unreachable!(),
        };
        let response = fetch(&self.current_url, selector.as_str(), ssl, protocol);
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
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            // Navigation and address bar
            ui.horizontal(|ui| {
                if ui.add_enabled(can_go_back(), Button::new("Back")).clicked() {
                    if let Some(entry) = history::back() {
                        self.url.set(entry.url.to_string());
                        self.navigate(Some(entry.protocol), false);
                    }
                }
                if ui
                    .add_enabled(can_go_forward(), Button::new("Forward"))
                    .clicked()
                {
                    if let Some(entry) = history::forward() {
                        self.url.set(entry.url.to_string());
                        self.navigate(Some(entry.protocol), false);
                    }
                }
                let url = ui.text_edit_singleline(self.url.get_mut());
                if url.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                    self.navigate(None, true);
                }
                if ui.button("Go").clicked() {
                    self.navigate(None, true);
                }
            });
            ui.separator();

            // Page content
            let mut scroll_area = ScrollArea::both().auto_shrink(false);
            if self.reset_scroll_pos {
                scroll_area = scroll_area.scroll_offset([0.0, 0.0].into());
                self.reset_scroll_pos = false;
            }
            scroll_area.show(ui, |ui| {
                let protocol = Protocol::from_url(&self.current_url);
                match protocol {
                    Protocol::Finger => self.content_handlers.finger.render_page(ui, self),
                    Protocol::Gemini | Protocol::Spartan | Protocol::Guppy => {
                        self.content_handlers.gemtext.render_page(ui, self);
                    }
                    Protocol::Gopher(_) => self.content_handlers.gopher.render_page(ui, self),
                    _ => self.content_handlers.plaintext.render_page(ui, self),
                }
            });
        });

        if let Some((_, protocol)) = self.navigation_hint.take() {
            self.navigate(Some(protocol), true);
            self.reset_scroll_pos = true;
        }
    }
}
