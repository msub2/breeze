#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod db;
mod handlers;
mod history;
mod networking;
mod profile;

use std::cell::{Cell, RefCell};
use std::process::exit;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Parser;
use db::{get_all_profiles, set_active_profile};
use eframe::egui::{
    include_image, menu, vec2, Align, Button, CentralPanel, Context, CursorIcon, FontData,
    FontDefinitions, FontFamily, Frame, IconData, Image, Key, Label, Layout, Modal, PointerButton,
    RichText, ScrollArea, Separator, TextEdit, TopBottomPanel, Ui, ViewportBuilder, ViewportId,
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
use crate::profile::Profile;

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
        .with_inner_size([800.0, 600.0])
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
    pub completed: bool,
}

enum ActiveView {
    Browser,
    Mail,
    Chat,
    Composer,
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
    show_about_window: Arc<AtomicBool>,
    status_text: RefCell<String>,
    active_view: ActiveView,
    profiles: Vec<Profile>,
    should_update_profiles: bool,
}

impl Breeze {
    fn new(starting_url: String) -> Self {
        let starting_url = Url::from_str(&starting_url).unwrap();
        let profiles = get_all_profiles().unwrap();
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
            show_about_window: Arc::new(AtomicBool::new(false)),
            status_text: RefCell::new("".to_string()),
            active_view: ActiveView::Browser,
            profiles,
            should_update_profiles: false,
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
        let plaintext = protocol_hint.is_some_and(|p| p == Protocol::Plaintext)
            || current_url.ends_with(".txt");
        let (request_body, ssl) = match protocol {
            Protocol::Finger => (path.strip_prefix("/").unwrap_or(&path).to_string(), false),
            Protocol::Gemini => (current_url, true),
            Protocol::Gopher(ssl) => (format!("{}{}", path, query), ssl),
            Protocol::Guppy => (current_url, false),
            Protocol::Nex => (path, false),
            Protocol::Scorpion => (format!("R {}", current_url), false),
            Protocol::Scroll => (format!("{} {}", current_url, "en"), true),
            Protocol::Spartan => {
                let query = if let Some(q) = self.current_url.query() {
                    &format!("{}\n{}", q.len(), q)
                } else {
                    "0"
                };
                (format!("{} {} {}", hostname, path, query), false)
            }
            Protocol::TextProtocol => (current_url, false),
            _ => unreachable!(),
        };
        let url = self.current_url.clone();
        let promise =
            Promise::spawn_thread("net", move || fetch(&url, &request_body, ssl, protocol));
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
                ui.menu_button("Profile", |ui| {
                    if ui.button("New").clicked() {
                        self.input_request = Some(InputRequest {
                            prompt: "Enter the name you would like to use for this profile"
                                .to_string(),
                            sensitive: false,
                            destination: "breeze://profile/new".to_string(),
                            user_input: String::new(),
                            completed: false,
                        })
                    }
                    ui.separator();
                    for profile in &self.profiles {
                        ui.horizontal(|ui| {
                            if profile.active {
                                ui.label("✓");
                            }

                            let button = Button::new(&profile.name).min_size([128.0, 20.0].into());
                            if ui.add(button).clicked() {
                                let _ = set_active_profile(profile.name.clone());
                                self.should_update_profiles = true;
                            }
                        });
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About Breeze").clicked() {
                        self.show_about_window.store(true, Ordering::Relaxed);
                    }
                });
            });
        });
        TopBottomPanel::bottom("statusbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Browser").clicked() {
                    self.active_view = ActiveView::Browser;
                }
                if ui.button("Mail").clicked() {
                    self.active_view = ActiveView::Mail;
                }
                if ui.button("Chat").clicked() {
                    self.active_view = ActiveView::Chat;
                }
                if ui.button("Composer").clicked() {
                    self.active_view = ActiveView::Composer;
                }
                ui.separator();
                ui.add_sized(
                    ui.available_size(),
                    Label::new(self.status_text.borrow().clone()),
                );
            });
        });
        self.status_text.borrow_mut().clear();
        CentralPanel::default().show(ctx, |ui| match self.active_view {
            ActiveView::Browser => render_browser(ui, ctx, self),
            ActiveView::Mail => render_mail(ui, ctx, self),
            ActiveView::Chat => render_chat(ui, ctx, self),
            ActiveView::Composer => render_composer(ui, ctx, self),
        });

        if self.show_about_window.load(Ordering::Relaxed) {
            let show_about_window = self.show_about_window.clone();
            ctx.show_viewport_deferred(
                ViewportId::from_hash_of("about"),
                ViewportBuilder::default()
                    .with_title("About")
                    .with_inner_size([640.0, 480.0]),
                move |ctx, _| {
                    CentralPanel::default().show(ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add(
                                Image::new(include_image!("../res/breeze512.png"))
                                    .max_size([256.0, 256.0].into()),
                            );
                            ui.add(Label::new(RichText::new("Breeze").size(24.0)));
                            ui.label("Version: 0.0.1-alpha");
                        });
                    });

                    if ctx.input(|i| i.viewport().close_requested()) {
                        show_about_window.store(false, Ordering::Relaxed);
                    }
                },
            );
        }

        if let Some(hint) = self.navigation_hint.take() {
            self.url.set(hint.url);
            self.navigate(Some(hint.protocol), hint.add_to_history);
            self.reset_scroll_pos = true;
        }

        if self.input_request.as_ref().is_some_and(|r| r.completed) {
            self.input_request = None;
        }

        if self.should_update_profiles {
            self.should_update_profiles = false;
            self.profiles = get_all_profiles().unwrap();
        }

        let Some(job) = &self.nav_job else { return };
        match job.nav_promise.ready() {
            Some(Ok(response)) => {
                // TODO: This feels like it's getting very verbose,
                // see if there's a way to better work with these statuses
                self.page_content = String::from_utf8_lossy(&response.content).to_string();
                match &response.status {
                    // Input
                    ServerStatus::Gemini(GeminiStatus::InputExpected(prompt, sensitive)) => {
                        history::remove_latest_entry();
                        self.input_request = Some(InputRequest {
                            prompt: prompt.clone(),
                            sensitive: *sensitive,
                            destination: self.current_url.to_string(),
                            user_input: "".to_string(),
                            completed: false,
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
                    | ServerStatus::TextProtocol(TextProtocolStatus::Redirect(url))
                    | ServerStatus::Scorpion(ScorpionStatus::TemporaryRedirect(url))
                    | ServerStatus::Scorpion(ScorpionStatus::PermanentRedirect(url)) => {
                        println!("Redirecting to: {}", url);
                        let mut current_url = self.current_url.clone();
                        if url.starts_with("/") {
                            current_url.set_path(&url);
                        } else {
                            current_url.join(&url).unwrap();
                        }
                        self.url.set(current_url.to_string());
                        self.navigation_hint.set(Some(NavigationHint {
                            url: current_url.to_string(),
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
                    // Certificates
                    ServerStatus::Gemini(GeminiStatus::RequiresClientCertificate) => {
                        let msg = format!("The requested resource requires a client certificate. You can create one by clicking \"New\" in the Profiles tab.");
                        self.content_handlers
                            .parse_content(msg.as_bytes(), true, job.protocol);
                    }
                    ServerStatus::Gemini(GeminiStatus::CertificateNotAuthorized) => {
                        let msg = format!(
                            "Your client certificate is not authorized to access this resource"
                        );
                        self.content_handlers
                            .parse_content(msg.as_bytes(), true, job.protocol);
                    }
                    ServerStatus::Gemini(GeminiStatus::CertificateNotValid) => {
                        let msg = format!("The requested resource is unavailable as your client certificate is invalid. Check to see if your certificate has expired.");
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

fn render_browser(ui: &mut eframe::egui::Ui, ctx: &Context, breeze: &mut Breeze) {
    // Navigation and address bar
    ui.horizontal(|ui| {
        if ui.add_enabled(can_go_back(), Button::new("←")).clicked()
            || ui.input(|input| input.pointer.button_clicked(PointerButton::Extra1))
        {
            if let Some(entry) = history::back() {
                breeze.url.set(entry.url.to_string());
                breeze.navigate(Some(entry.protocol), false);
            }
        }
        if ui.add_enabled(can_go_forward(), Button::new("→")).clicked()
            || ui.input(|input| input.pointer.button_clicked(PointerButton::Extra2))
        {
            if let Some(entry) = history::forward() {
                breeze.url.set(entry.url.to_string());
                breeze.navigate(Some(entry.protocol), false);
            }
        }
        // Layout trick to have address bar render last and fill available remaining space
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Go").clicked() {
                breeze.navigate(None, true);
            }
            let url = ui.add_sized(
                ui.available_size(),
                TextEdit::singleline(breeze.url.get_mut()),
            );
            if url.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                breeze.navigate(None, true);
            }
        });
    });
    // Extend separator out a bit to match menubar separator
    ui.add(Separator::default().grow(8.0));

    // Page content
    let mut scroll_area = ScrollArea::both().auto_shrink(false);
    if breeze.reset_scroll_pos {
        scroll_area = scroll_area.scroll_offset([0.0, 0.0].into());
        breeze.reset_scroll_pos = false;
    }
    scroll_area.show(ui, |ui| {
        Frame::new().inner_margin(vec2(64.0, 16.0)).show(ui, |ui| {
            // TODO: This should eventually check content type instead of protocol
            let protocol = Protocol::from_url(&breeze.current_url);
            match protocol {
                Protocol::Finger => breeze.content_handlers.finger.render_page(ui, breeze),
                Protocol::Gemini | Protocol::Spartan | Protocol::Guppy | Protocol::Scroll => {
                    breeze.content_handlers.gemtext.render_page(ui, breeze);
                }
                Protocol::Gopher(_) => breeze.content_handlers.gopher.render_page(ui, breeze),
                Protocol::Nex => breeze.content_handlers.nex.render_page(ui, breeze),
                Protocol::Scorpion => breeze.content_handlers.scorpion.render_page(ui, breeze),
                _ => breeze.content_handlers.plaintext.render_page(ui, breeze),
            }
        })
    });

    if let Some(input_request) = &mut breeze.input_request {
        Modal::new("input".into()).show(ctx, |ui| {
            ui.label(input_request.prompt.as_str());
            let text_edit = TextEdit::singleline(&mut input_request.user_input)
                .password(input_request.sensitive);
            ui.add(text_edit);
            if ui.button("Submit").clicked() {
                if input_request.destination == "breeze://profile/new" {
                    let profile = Profile::new(input_request.user_input.clone());
                    let currently_active = breeze.profiles.iter_mut().find(|p| p.active);
                    if let Some(currently_active) = currently_active {
                        currently_active.active = false;
                    }
                    breeze.profiles.push(profile);
                } else {
                    let url = format!("{}?{}", input_request.destination, input_request.user_input);
                    breeze.navigation_hint.set(Some(NavigationHint {
                        url,
                        protocol: Protocol::from_str(&input_request.destination),
                        add_to_history: true,
                    }));
                }
                input_request.completed = true;
            }
        });
    }
}

fn render_mail(ui: &mut Ui, _ctx: &Context, _breeze: &mut Breeze) {
    ui.label("This is a placeholder for the mail tab, which will act as a client for Misfin and the NPS.");
}

fn render_chat(ui: &mut Ui, _ctx: &Context, _breeze: &mut Breeze) {
    ui.label("This is a placeholder for the chat tab, which will feature a built-in IRC client.");
}

fn render_composer(ui: &mut Ui, _ctx: &Context, _breeze: &mut Breeze) {
    ui.label("This is a placeholder for the composer tab, which will allow users to compose Gemtext, Gophermaps, and so on.");
}
