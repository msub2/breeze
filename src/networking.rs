use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::{LazyLock, Mutex};

use eframe::egui::TextBuffer;
use native_tls::TlsConnector;
use url::Url;

use crate::db::get_default_profile;
use crate::handlers::Protocol;

#[allow(dead_code)]
static DNS_CACHE: LazyLock<Mutex<HashMap<String, Vec<SocketAddr>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[allow(dead_code)]
pub fn is_hostname_valid(hostname: &str) -> bool {
    let mut cache = DNS_CACHE.lock().expect("Failed to lock DNS cache");
    if cache.contains_key(hostname) {
        cache.get(hostname).is_some_and(|addrs| !addrs.is_empty())
    } else {
        // println!("First time seeing host: {}", hostname);
        match hostname.to_socket_addrs() {
            Ok(addrs) => {
                cache.insert(hostname.to_string(), addrs.collect::<Vec<_>>());
                true
            }
            Err(_) => {
                cache.insert(hostname.to_string(), Vec::new());
                false
            }
        }
    }
}

// TODO: Gopher+

#[derive(Debug)]
pub enum GeminiStatus {
    InputExpected(String, bool),
    Success(String),
    TemporaryRedirect(String),
    PermanentRedirect(String),
    TemporaryFailure(String),
    ServerUnavailable(String),
    CGIError(String),
    ProxyError(String),
    SlowDown(String),
    PermanentFailure(String),
    NotFound(String),
    Gone(String),
    ProxyRequestRefused(String),
    BadRequest(String),
    RequiresClientCertificate,
    CertificateNotAuthorized,
    CertificateNotValid,
}

impl From<&str> for GeminiStatus {
    fn from(status: &str) -> Self {
        let (code, data) = status.split_once(' ').unwrap();
        let data = data.to_string();
        match code {
            "10" => GeminiStatus::InputExpected(data, false),
            "11" => GeminiStatus::InputExpected(data, true),
            // Most of these extra ones are for scroll
            "20" | "21" | "22" | "23" | "24" | "25" | "26" | "27" | "28" | "29" => {
                GeminiStatus::Success(data)
            }
            "30" => GeminiStatus::TemporaryRedirect(data),
            "31" => GeminiStatus::PermanentRedirect(data),
            "40" => GeminiStatus::TemporaryFailure(data),
            "41" => GeminiStatus::ServerUnavailable(data),
            "42" => GeminiStatus::CGIError(data),
            "43" => GeminiStatus::ProxyError(data),
            "44" => GeminiStatus::SlowDown(data),
            "50" => GeminiStatus::PermanentFailure(data),
            "51" => GeminiStatus::NotFound(data),
            "52" => GeminiStatus::Gone(data),
            "53" => GeminiStatus::ProxyRequestRefused(data),
            "59" => GeminiStatus::BadRequest(data),
            "60" => GeminiStatus::RequiresClientCertificate,
            "61" => GeminiStatus::CertificateNotAuthorized,
            "62" => GeminiStatus::CertificateNotValid,
            _ => unreachable!("Unknown Gemini status code: {}", code),
        }
    }
}

#[derive(Debug)]
pub enum SpartanStatus {
    Success(String),
    Redirect(String),
    ClientError(String),
    ServerError(String),
}

impl From<&str> for SpartanStatus {
    fn from(status: &str) -> Self {
        let (code, data) = status.split_once(' ').unwrap();
        match code {
            "2" => SpartanStatus::Success(data.to_string()),
            "3" => SpartanStatus::Redirect(data.to_string()),
            "4" => SpartanStatus::ClientError(data.to_string()),
            "5" => SpartanStatus::ServerError(data.to_string()),
            _ => unreachable!("Unknown Spartan status code: {}", code),
        }
    }
}

#[derive(Debug)]
pub enum ScorpionStatus {
    Interactive,
    InputRequired,
    OK,
    PartialOK,
    TemporaryRedirect(String),
    PermanentRedirect(String),
    TemporaryError,
    DownForMaintenance,
    DynamicFileError,
    ProxyError,
    SlowDown,
    TemporarilyLockedFile,
    PermanentError(String),
    FileNotFound(String),
    FileRemoved(String),
    ProxyRequestRefused,
    Forbidden,
    EditConflict,
    CredentialsRequired,
    BadRequest,
    RequiresClientCertificate,
    CertificateNotAuthorized,
    CertificateNotValid,
    ReadyNewFile,
    ReadyModifyFile,
    ReadyOther,
    AcceptedNewFile,
    AcceptedFileModified,
    AcceptedOther,
}

impl From<&str> for ScorpionStatus {
    fn from(status: &str) -> Self {
        // TODO: Handle additional data with codes
        let (code, data) = status.split_once(' ').unwrap();
        let data = data.to_string();
        match code {
            "00" => ScorpionStatus::Interactive,
            "10" => ScorpionStatus::InputRequired,
            "20" => ScorpionStatus::OK,
            "21" => ScorpionStatus::PartialOK,
            "30" => ScorpionStatus::TemporaryRedirect(data),
            "31" => ScorpionStatus::PermanentRedirect(data),
            "40" => ScorpionStatus::TemporaryError,
            "41" => ScorpionStatus::DownForMaintenance,
            "42" => ScorpionStatus::DynamicFileError,
            "43" => ScorpionStatus::ProxyError,
            "44" => ScorpionStatus::SlowDown,
            "45" => ScorpionStatus::TemporarilyLockedFile,
            "50" => ScorpionStatus::PermanentError(data),
            "51" => ScorpionStatus::FileNotFound(data),
            "52" => ScorpionStatus::FileRemoved(data),
            "53" => ScorpionStatus::ProxyRequestRefused,
            "54" => ScorpionStatus::Forbidden,
            "55" => ScorpionStatus::EditConflict,
            "56" => ScorpionStatus::CredentialsRequired,
            "59" => ScorpionStatus::BadRequest,
            "60" => ScorpionStatus::RequiresClientCertificate,
            "61" => ScorpionStatus::CertificateNotAuthorized,
            "62" => ScorpionStatus::CertificateNotValid,
            "70" => ScorpionStatus::ReadyNewFile,
            "71" => ScorpionStatus::ReadyModifyFile,
            "72" => ScorpionStatus::ReadyOther,
            "80" => ScorpionStatus::AcceptedNewFile,
            "81" => ScorpionStatus::AcceptedFileModified,
            "82" => ScorpionStatus::AcceptedOther,
            _ => unreachable!("Unknown Scorpion status code: {}", status),
        }
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug)]
pub enum TextProtocolStatus {
    OK(String),
    Redirect(String),
    NOK(String),
}

impl From<&str> for TextProtocolStatus {
    fn from(status: &str) -> Self {
        let (code, data) = status.split_once(' ').unwrap();
        match code {
            "20" => TextProtocolStatus::OK(data.to_string()),
            "30" => TextProtocolStatus::Redirect(data.to_string()),
            "40" => TextProtocolStatus::NOK(data.to_string()),
            _ => unreachable!("Unknown Text Protocol status code: {}", code),
        }
    }
}

#[derive(Debug)]
pub enum ServerStatus {
    Gemini(GeminiStatus),
    Scorpion(ScorpionStatus),
    Spartan(SpartanStatus),
    TextProtocol(TextProtocolStatus),
    _Success(String),
}

#[derive(Debug)]
pub struct ServerResponse {
    pub content: Vec<u8>,
    pub status: ServerStatus,
}

pub fn fetch(
    url: &Url,
    request_body: &str,
    ssl: bool,
    protocol: Protocol,
) -> Result<ServerResponse, String> {
    let hostname = url.host_str().expect("Hostname is empty!");
    let port = url.port().unwrap_or(match protocol {
        Protocol::Finger => 79,
        Protocol::Gemini => 1965,
        Protocol::Gopher(_) => 70,
        Protocol::Guppy => 6775,
        Protocol::Nex => 1900,
        Protocol::Scorpion => 1517,
        Protocol::Scroll => 5699,
        Protocol::Spartan => 300,
        Protocol::TextProtocol => 1961,
        _ => 0,
    });
    let url = format!("{}:{}", hostname, port);
    let request = format!("{}\r\n", request_body);
    let mut buf = Vec::new();

    if protocol == Protocol::Guppy {
        return fetch_udp(hostname, port, request_body, ssl);
    }

    if ssl {
        let identity = match get_default_profile() {
            Ok(p) => Some(p.identity),
            Err(_) => None,
        };
        let mut connector_builder = TlsConnector::builder();
        connector_builder.danger_accept_invalid_certs(true);
        if let Some(identity) = identity {
            connector_builder.identity(identity);
        }
        let connector = connector_builder.build().unwrap();

        let stream =
            TcpStream::connect(format!("{}:{}", hostname, port)).map_err(|e| e.to_string())?;
        let mut stream = connector
            .connect(hostname, stream)
            .map_err(|e| e.to_string())?;

        stream
            .write_all(request.as_bytes())
            .map_err(|e| e.to_string())?;
        stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        Ok(parse_server_response(&buf, protocol))
    } else if let Ok(mut stream) = TcpStream::connect(url) {
        stream
            .write_all(request.as_bytes())
            .map_err(|e| e.to_string())?;
        stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        Ok(parse_server_response(&buf, protocol))
    } else {
        let msg = format!("Failed to connect to hostname: {}", hostname);
        Err(msg)
    }
}

fn fetch_udp(
    hostname: &str,
    port: u16,
    selector: &str,
    _ssl: bool,
) -> Result<ServerResponse, String> {
    let url = format!("{}:{}", hostname, port);
    let request = format!("{}\r\n", selector);
    let mut data = Vec::new();
    let mut completed = false;

    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        let addrs = url.to_socket_addrs().unwrap().collect::<Vec<_>>();
        socket.connect(addrs.first().unwrap()).unwrap();
        socket.send(request.as_bytes()).map_err(|e| e.to_string())?;
        while !completed {
            // This code is all Guppy-specific, as it's the only protocol using UDP instead of TCP
            let mut buf = [0; 16384];
            socket.recv(buf.as_mut()).map_err(|e| e.to_string())?;
            let first_line = buf.lines().next().unwrap().unwrap();
            let server_info = first_line.split(' ').collect::<Vec<_>>();
            if let Some(content_type) = server_info.get(1) {
                data.extend_from_slice(format!("{}\n", content_type).as_bytes());
            }
            if let Some(seq) = server_info.first() {
                let data_lines = buf.lines().skip(1).collect::<Vec<_>>();
                if data_lines.len() == 1 {
                    completed = true;
                } else {
                    let newbuf = buf
                        .iter()
                        .filter_map(|b| if *b != 0 { Some(*b) } else { None })
                        .collect::<Vec<_>>();
                    let string = String::from_utf8_lossy(&newbuf).to_string();
                    let newstring = string.split_once("\n").unwrap().1;
                    data.extend(newstring.as_bytes());
                    socket
                        .send(format!("{}\r\n", seq).as_bytes())
                        .map_err(|e| e.to_string())?;
                }
            } else {
                completed = true;
            }
        }
        Ok(parse_server_response(&data, Protocol::Guppy))
    } else {
        Err(format!("Failed to connect to hostname: {}", hostname))
    }
}

fn parse_server_response(response: &[u8], protocol: Protocol) -> ServerResponse {
    match protocol {
        Protocol::Gemini | Protocol::Scroll => {
            let response = String::from_utf8_lossy(response);
            let (server_status, content) = response.split_once('\n').unwrap();
            ServerResponse {
                content: Vec::from(content),
                status: ServerStatus::Gemini(GeminiStatus::from(server_status)),
            }
        }
        Protocol::Guppy => {
            let response = String::from_utf8_lossy(response);
            let (content_type, content) = response.split_once('\n').unwrap();
            ServerResponse {
                content: Vec::from(content),
                status: ServerStatus::_Success(content_type.to_string()),
            }
        }
        Protocol::TextProtocol => {
            let response = String::from_utf8_lossy(response);
            let (server_status, content) = response.split_once('\n').unwrap();
            ServerResponse {
                content: Vec::from(content),
                status: ServerStatus::TextProtocol(TextProtocolStatus::from(server_status)),
            }
        }
        Protocol::Scorpion => {
            // Get status line from server response
            let status_end = response.iter().position(|b| *b == b'\n').unwrap();
            let status_line_bytes = &response[0..status_end];
            let content_bytes = &response[status_end + 1..];
            let status_line = String::from_utf8_lossy(status_line_bytes);

            // Parse status line
            let status = ScorpionStatus::from(status_line.as_str());

            ServerResponse {
                content: Vec::from(content_bytes),
                status: ServerStatus::Scorpion(status),
            }
        }
        Protocol::Spartan => {
            let response = String::from_utf8_lossy(response);
            let (server_status, content) = response.split_once('\n').unwrap();
            ServerResponse {
                content: Vec::from(content),
                status: ServerStatus::Spartan(SpartanStatus::from(server_status)),
            }
        }
        _ => ServerResponse {
            content: response.to_owned(),
            status: ServerStatus::_Success("text/plain".to_string()),
        },
    }
}
