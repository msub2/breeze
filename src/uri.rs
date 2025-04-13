use crate::protocols::Protocol;

pub fn get_protocol(scheme: &str) -> Protocol {
    match scheme {
        "finger" => Protocol::Finger,
        "gemini" => Protocol::Gemini,
        "gopher" => Protocol::Gopher,
        "scorpion" => Protocol::Scorpion,
        _ => Protocol::Unknown,
    }
}
