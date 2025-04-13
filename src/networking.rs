use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::{LazyLock, Mutex};

static DNS_CACHE: LazyLock<Mutex<HashMap<String, Vec<SocketAddr>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

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

pub fn fetch(hostname: &str, port: u16, selector: &str) -> String {
    let url = format!("{}:{}", hostname, port);
    let request = format!("{}\r\n", selector);

    let mut stream = TcpStream::connect(url).unwrap();
    stream.write(request.as_bytes()).unwrap();
    let mut buf = String::new();
    stream.read_to_string(&mut buf).unwrap();

    // println!("{}", buf);

    buf
}
