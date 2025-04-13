use crate::protocols::Protocol;

pub fn get_protocol(scheme: &str) -> Protocol {
    match scheme {
        "gopher" => Protocol::Gopher,
        _ => Protocol::Unknown,
    }
}
