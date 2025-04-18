use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::{LazyLock, Mutex};

use native_tls::TlsConnector;
use url::Url;

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
    InputExpected(String),
    SensitiveInputExpected(String),
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
            "10" => GeminiStatus::InputExpected(data),
            "11" => GeminiStatus::SensitiveInputExpected(data),
            "20" => GeminiStatus::Success(data),
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

// TODO: Scorpion

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
    Spartan(SpartanStatus),
    TextProtocol(TextProtocolStatus),
    _Success(String),
}

#[derive(Debug)]
pub struct ServerResponse {
    pub content: String,
    pub status: ServerStatus,
}

pub fn fetch(
    url: &Url,
    selector: &str,
    ssl: bool,
    protocol: Protocol,
) -> Result<ServerResponse, String> {
    let hostname = url.host_str().expect("Hostname is empty!");
    let port = url.port().unwrap_or_else(|| match protocol {
        Protocol::Finger => 79,
        Protocol::Gemini => 1965,
        Protocol::Gopher(_) => 70,
        Protocol::Guppy => 6775,
        Protocol::Nex => 1900,
        Protocol::Scorpion => 1517,
        Protocol::Spartan => 300,
        Protocol::TextProtocol => 1961,
        _ => 0,
    });
    let url = format!("{}:{}", hostname, port);
    let request = format!("{}\r\n", selector);
    let mut buf = String::new();

    if protocol == Protocol::Guppy {
        return fetch_udp(hostname, port, selector, ssl);
    }

    if ssl {
        let connector = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        let stream = TcpStream::connect(format!("{}:{}", hostname, port)).unwrap();
        let mut stream = connector.connect(hostname, stream).unwrap();

        stream
            .write_all(request.as_bytes())
            .map_err(|e| e.to_string())?;
        stream.read_to_string(&mut buf).map_err(|e| e.to_string())?;
        Ok(parse_server_response(&buf, protocol))
    } else if let Ok(mut stream) = TcpStream::connect(url) {
        stream
            .write_all(request.as_bytes())
            .map_err(|e| e.to_string())?;
        stream.read_to_string(&mut buf).map_err(|e| e.to_string())?;
        Ok(parse_server_response(&buf, protocol))
    } else {
        buf.push_str(&format!("Failed to connect to hostname: {}", hostname));
        Err(buf)
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
            let mut buf = [0; 16384];
            socket.recv(buf.as_mut()).map_err(|e| e.to_string())?;
            let first_line = buf.lines().next().unwrap().unwrap();
            let server_info = first_line.split(' ').collect::<Vec<_>>();
            if let Some(content_type) = server_info.get(1) {
                // TODO: This is a hack, need to properly respond to server codes when needed
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
        let response = String::from_utf8_lossy(&data).to_string();
        Ok(parse_server_response(&response, Protocol::Guppy))
    } else {
        Err(format!("Failed to connect to hostname: {}", hostname))
    }
}

fn parse_server_response(response: &str, protocol: Protocol) -> ServerResponse {
    match protocol {
        Protocol::Gemini => {
            let (server_status, content) = response.split_once('\n').unwrap();
            ServerResponse {
                content: content.to_string(),
                status: ServerStatus::Gemini(GeminiStatus::from(server_status)),
            }
        }
        Protocol::Guppy => {
            let (content_type, content) = response.split_once('\n').unwrap();
            ServerResponse {
                content: content.to_string(),
                status: ServerStatus::_Success(content_type.to_string()),
            }
        }
        Protocol::TextProtocol => {
            let (server_status, content) = response.split_once('\n').unwrap();
            ServerResponse {
                content: content.to_string(),
                status: ServerStatus::TextProtocol(TextProtocolStatus::from(server_status)),
            }
        }
        Protocol::Spartan => {
            let (server_status, content) = response.split_once('\n').unwrap();
            ServerResponse {
                content: content.to_string(),
                status: ServerStatus::Spartan(SpartanStatus::from(server_status)),
            }
        }
        _ => ServerResponse {
            content: response.to_string(),
            status: ServerStatus::_Success("text/plain".to_string()),
        },
    }
}
