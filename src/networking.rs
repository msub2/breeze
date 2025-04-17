use std::collections::HashMap;
use std::io::{BufRead, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::{LazyLock, Mutex};

use native_tls::TlsConnector;
use url::Url;

use crate::protocols::Protocol;

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

pub fn fetch(url: &Url, selector: &str, ssl: bool, protocol: Protocol) -> Result<String, String> {
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
        Ok(buf)
    } else if let Ok(mut stream) = TcpStream::connect(url) {
        stream
            .write_all(request.as_bytes())
            .map_err(|e| e.to_string())?;
        stream.read_to_string(&mut buf).map_err(|e| e.to_string())?;
        Ok(buf)
    } else {
        buf.push_str(&format!("Failed to connect to hostname: {}", hostname));
        Err(buf)
    }
}

fn fetch_udp(hostname: &str, port: u16, selector: &str, _ssl: bool) -> Result<String, String> {
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
        Ok(String::from_utf8_lossy(&data).to_string())
    } else {
        Err(format!("Failed to connect to hostname: {}", hostname))
    }
}
