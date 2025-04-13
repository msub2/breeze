use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{LazyLock, Mutex};

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

pub fn fetch(hostname: &str, port: u16, selector: &str) -> Result<String, String> {
    let url = format!("{}:{}", hostname, port);
    let request = format!("{}\r\n", selector);
    let mut buf = String::new();

    if let Ok(mut stream) = TcpStream::connect(url) {
        stream.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
        stream.read_to_string(&mut buf).map_err(|e| e.to_string())?;
        Ok(buf)
    } else {
        buf.push_str(&format!("Failed to connect to hostname: {}", hostname));
        Err(buf)
    }
}
