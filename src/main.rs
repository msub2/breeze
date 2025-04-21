#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod handlers;
mod history;
mod networking;

use std::cell::Cell;
use std::process::exit;
use std::str::FromStr;
use std::sync::Arc;

use clap::Parser;
use eframe::egui::{
    menu, Align, Button, CentralPanel, Context, CursorIcon, FontData, FontDefinitions, FontFamily,
    IconData, Key, Layout, Modal, PointerButton, ScrollArea, Separator, TextEdit, TopBottomPanel,
    ViewportBuilder,
};
use poll_promise::Promise;
use url::Url;

use crate::handlers::finger::Finger;
use crate::handlers::gemtext::Gemtext;
use crate::handlers::gopher::Gopher;
use crate::handlers::nex::Nex;
use crate::handlers::plaintext::Plaintext;
use crate::handlers::scorpion::Scorpion;
use crate::handlers::{Protocol, ProtocolHandler};
use crate::history::{add_entry, can_go_back, can_go_forward};
use crate::networking::{
    fetch, GeminiStatus, ScorpionStatus, ServerResponse, ServerStatus, SpartanStatus,
    TextProtocolStatus,
};

#[derive(Parser)]
struct Args {
    #[arg(short, long, default_value = "gemini://geminiprotocol.net/")]
    url: String,
}

fn main() -> eframe::Result {
    let args = Args::parse();
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
    // UnifontEX for uniform monospace pan-unicode font
    load_font!(
        fonts,
        FontFamily::Monospace,
        "UnifontEx".to_string(),
        "../res/UnifontExMono.ttf"
    );
    // Code2000 for proportional pan-unicode font
    load_font!(
        fonts,
        FontFamily::Proportional,
        "CODE2000".to_string(),
        "../res/CODE2000.ttf"
    );

    eframe::run_native(
        "Breeze",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::<Breeze>::new(Breeze::new(args.url)))
        }),
    )
}

#[macro_export]
macro_rules! load_font {
    ($fonts:expr, $font_family:expr, $font_name:expr, $font_path:expr) => {
        $fonts.font_data.insert(
            $font_name.clone(),
            Arc::new(FontData::from_static(include_bytes!($font_path))),
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
    gemtext: Gemtext,
    gopher: Gopher,
    nex: Nex,
    scorpion: Scorpion,
    plaintext: Plaintext,
}

impl ContentHandlers {
    pub fn parse_content(&mut self, response: &[u8], plaintext: bool, protocol: Protocol) {
        match protocol {
            Protocol::Finger => self.finger.parse_content(response, plaintext),
            Protocol::Gemini | Protocol::Spartan | Protocol::Guppy | Protocol::Scroll => {
                self.gemtext.parse_content(response, plaintext)
            }
            Protocol::Gopher(_) => self.gopher.parse_content(response, plaintext),
            Protocol::Nex => self.nex.parse_content(response, plaintext),
            Protocol::Scorpion => self.scorpion.parse_content(response, plaintext),
            _ => self.plaintext.parse_content(response, plaintext),
        }
    }
}

struct NavigationHint {
    pub url: String,
    pub protocol: Protocol,
    pub add_to_history: bool,
}

struct NavigationJob {
    nav_promise: Promise<Result<ServerResponse, String>>,
    plaintext: bool,
    protocol: Protocol,
}

impl NavigationJob {
    fn new(
        nav_promise: Promise<Result<ServerResponse, String>>,
        plaintext: bool,
        protocol: Protocol,
    ) -> Self {
        Self {
            nav_promise,
            plaintext,
            protocol,
        }
    }
}

struct InputRequest {
    pub prompt: String,
    pub sensitive: bool,
    pub destination: String,
    pub user_input: String,
}

struct Breeze {
    /// The current value of the URL bar
    url: Cell<String>,
    /// The last URL that was navigated to
    current_url: Url,
    /// The plaintext response from the server for this page
    page_content: String,
    content_handlers: ContentHandlers,
    navigation_hint: Cell<Option<NavigationHint>>,
    reset_scroll_pos: bool,
    nav_job: Option<NavigationJob>,
    input_request: Option<InputRequest>,
}

impl Breeze {
    fn new(starting_url: String) -> Self {
        let starting_url = Url::from_str(&starting_url).unwrap();
        Self {
            url: Cell::new(starting_url.to_string()),
            current_url: starting_url.clone(),
            page_content: "".to_string(),
            content_handlers: Default::default(),
            navigation_hint: Cell::new(Some(NavigationHint {
                url: starting_url.to_string(),
                protocol: Protocol::from_url(&starting_url),
                add_to_history: true,
            })),
            reset_scroll_pos: false,
            nav_job: None,
            input_request: None,
        }
    }

    // Validate URL before updating the currently active page content
    fn navigate(&mut self, protocol_hint: Option<Protocol>, should_add_entry: bool) {
        self.input_request = None;
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
        let plaintext = protocol_hint.is_some_and(|p| p == Protocol::Plaintext)
            || current_url.ends_with(".txt");
        let (selector, ssl) = match protocol {
            Protocol::Finger => (path.strip_prefix("/").unwrap_or(&path).to_string(), false),
            Protocol::Gemini => (current_url, true),
            Protocol::Gopher(ssl) => (format!("{}{}", path, query), ssl),
            Protocol::Guppy => (current_url, false),
            Protocol::Nex => (path, false),
            Protocol::Scorpion => (format!("R {}", current_url), false),
            Protocol::Scroll => (format!("{} {}", current_url, "en"), true),
            Protocol::Spartan => (format!("{} {} {}", hostname, path, 0), false),
            Protocol::TextProtocol => (current_url, false),
            _ => unreachable!(),
        };
        let url = self.current_url.clone();
        let promise =
            Promise::spawn_thread("net", move || fetch(&url, selector.as_str(), ssl, protocol));
        self.nav_job
            .replace(NavigationJob::new(promise, plaintext, protocol));
    }
}

impl eframe::App for Breeze {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        TopBottomPanel::top("menubar").show(ctx, |ui| {
            menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        exit(0);
                    }
                });
            });
        });
        CentralPanel::default().show(ctx, |ui| {
            // Navigation and address bar
            ui.horizontal(|ui| {
                if ui.add_enabled(can_go_back(), Button::new("Back")).clicked()
                    || ui.input(|input| input.pointer.button_clicked(PointerButton::Extra1))
                {
                    if let Some(entry) = history::back() {
                        self.url.set(entry.url.to_string());
                        self.navigate(Some(entry.protocol), false);
                    }
                }
                if ui
                    .add_enabled(can_go_forward(), Button::new("Forward"))
                    .clicked()
                    || ui.input(|input| input.pointer.button_clicked(PointerButton::Extra2))
                {
                    if let Some(entry) = history::forward() {
                        self.url.set(entry.url.to_string());
                        self.navigate(Some(entry.protocol), false);
                    }
                }
                // Layout trick to have address bar render last and fill available remaining space
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Go").clicked() {
                        self.navigate(None, true);
                    }
                    let url = ui.add_sized(
                        ui.available_size(),
                        TextEdit::singleline(self.url.get_mut()),
                    );
                    if url.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                        self.navigate(None, true);
                    }
                });
            });
            // Extend separator out a bit to match menubar separator
            ui.add(Separator::default().grow(8.0));

            // Page content
            let mut scroll_area = ScrollArea::both().auto_shrink(false);
            if self.reset_scroll_pos {
                scroll_area = scroll_area.scroll_offset([0.0, 0.0].into());
                self.reset_scroll_pos = false;
            }
            scroll_area.show(ui, |ui| {
                // TODO: This should eventually check content type instead of protocol
                let protocol = Protocol::from_url(&self.current_url);
                match protocol {
                    Protocol::Finger => self.content_handlers.finger.render_page(ui, self),
                    Protocol::Gemini | Protocol::Spartan | Protocol::Guppy | Protocol::Scroll => {
                        self.content_handlers.gemtext.render_page(ui, self);
                    }
                    Protocol::Gopher(_) => self.content_handlers.gopher.render_page(ui, self),
                    Protocol::Nex => self.content_handlers.nex.render_page(ui, self),
                    Protocol::Scorpion => self.content_handlers.scorpion.render_page(ui, self),
                    _ => self.content_handlers.plaintext.render_page(ui, self),
                }
            });

            if let Some(input_request) = &mut self.input_request {
                Modal::new("input".into()).show(ctx, |ui| {
                    ui.label(input_request.prompt.as_str());
                    let text_edit = TextEdit::singleline(&mut input_request.user_input)
                        .password(input_request.sensitive);
                    ui.add(text_edit);
                    if ui.button("Submit").clicked() {
                        let url =
                            format!("{}?{}", input_request.destination, input_request.user_input);
                        self.navigation_hint.set(Some(NavigationHint {
                            url,
                            protocol: Protocol::from_str(&input_request.destination),
                            add_to_history: true,
                        }));
                    }
                });
            }
        });

        if let Some(hint) = self.navigation_hint.take() {
            self.url.set(hint.url);
            self.navigate(Some(hint.protocol), hint.add_to_history);
            self.reset_scroll_pos = true;
        }

        let Some(job) = &self.nav_job else { return };
        match job.nav_promise.ready() {
            Some(Ok(response)) => {
                // TODO: This feels like it's getting very verbose,
                // see if there's a way to better work with these statuses
                match &response.status {
                    // Input
                    ServerStatus::Gemini(GeminiStatus::InputExpected(prompt, sensitive)) => {
                        history::remove_latest_entry();
                        self.input_request = Some(InputRequest {
                            prompt: prompt.clone(),
                            sensitive: *sensitive,
                            destination: self.current_url.to_string(),
                            user_input: "".to_string(),
                        });
                    }
                    // Success
                    ServerStatus::Gemini(GeminiStatus::Success(_content_type))
                    | ServerStatus::Spartan(SpartanStatus::Success(_content_type))
                    | ServerStatus::TextProtocol(TextProtocolStatus::OK(_content_type))
                    | ServerStatus::_Success(_content_type) => {
                        self.content_handlers.parse_content(
                            &response.content,
                            job.plaintext,
                            job.protocol,
                        );
                    }
                    ServerStatus::Scorpion(ScorpionStatus::OK) => {
                        self.content_handlers.parse_content(
                            &response.content,
                            job.plaintext,
                            job.protocol,
                        );
                    }
                    // Redirect
                    ServerStatus::Gemini(GeminiStatus::TemporaryRedirect(url))
                    | ServerStatus::Gemini(GeminiStatus::PermanentRedirect(url))
                    | ServerStatus::Spartan(SpartanStatus::Redirect(url))
                    | ServerStatus::TextProtocol(TextProtocolStatus::Redirect(url)) => {
                        println!("Redirecting to: {}", url);
                        self.url.set(url.clone());
                        self.navigation_hint.set(Some(NavigationHint {
                            url: url.clone(),
                            protocol: job.protocol,
                            add_to_history: true,
                        }));
                    }
                    // Failure
                    ServerStatus::Gemini(GeminiStatus::TemporaryFailure(data))
                    | ServerStatus::Gemini(GeminiStatus::ServerUnavailable(data))
                    | ServerStatus::Gemini(GeminiStatus::CGIError(data))
                    | ServerStatus::Gemini(GeminiStatus::ProxyError(data))
                    | ServerStatus::Gemini(GeminiStatus::SlowDown(data))
                    | ServerStatus::Gemini(GeminiStatus::PermanentFailure(data))
                    | ServerStatus::Gemini(GeminiStatus::NotFound(data))
                    | ServerStatus::Gemini(GeminiStatus::Gone(data))
                    | ServerStatus::Gemini(GeminiStatus::ProxyRequestRefused(data))
                    | ServerStatus::Gemini(GeminiStatus::BadRequest(data))
                    | ServerStatus::Scorpion(ScorpionStatus::PermanentError(data))
                    | ServerStatus::Scorpion(ScorpionStatus::FileNotFound(data))
                    | ServerStatus::Scorpion(ScorpionStatus::FileRemoved(data))
                    | ServerStatus::Spartan(SpartanStatus::ClientError(data))
                    | ServerStatus::Spartan(SpartanStatus::ServerError(data))
                    | ServerStatus::TextProtocol(TextProtocolStatus::NOK(data)) => {
                        let msg = format!("The requested resource could not be found.\n\nAdditional information:\n\n{}", data);
                        self.content_handlers
                            .parse_content(msg.as_bytes(), true, job.protocol);
                    }
                    _ => {
                        println!("Unhandled status: {:?}", response.status);
                    }
                }
                self.nav_job = None;
            }
            Some(Err(error)) => {
                self.content_handlers
                    .parse_content(error.as_bytes(), true, job.protocol);
                self.nav_job = None;
            }
            None => ctx.set_cursor_icon(CursorIcon::Wait),
        }
    }
}
