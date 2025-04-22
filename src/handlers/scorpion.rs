use eframe::egui::{Color32, CursorIcon, Label, RichText, Sense, Ui};

use crate::{Breeze, NavigationHint, Protocol};

use super::ProtocolHandler;

use codepage_437::{CP437_CONTROL, CP437_WINGDINGS};
use url::Url;

#[derive(Debug)]
enum BlockType {
    Paragraph,
    Heading1,
    Heading2,
    Heading3,
    Heading4,
    Heading5,
    Heading6,
    Hyperlink,
    HyperlinkInput,
    HyperlinkInteractive,
    AlternateService,
    Blockquote,
    Preformatted,
    Metadata,
}

impl From<u8> for BlockType {
    fn from(value: u8) -> Self {
        match value {
            0x00 => BlockType::Paragraph,
            0x01 => BlockType::Heading1,
            0x02 => BlockType::Heading2,
            0x03 => BlockType::Heading3,
            0x04 => BlockType::Heading4,
            0x05 => BlockType::Heading5,
            0x06 => BlockType::Heading6,
            0x08 => BlockType::Hyperlink,
            0x09 => BlockType::HyperlinkInput,
            0x0A => BlockType::HyperlinkInteractive,
            0x0B => BlockType::AlternateService,
            0x0C => BlockType::Blockquote,
            0x0D => BlockType::Preformatted,
            0x0F => BlockType::Metadata,
            _ => BlockType::Paragraph,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum CharacterEncoding {
    TRON8,
    PC,
    ISO2022,
    TRON8RTL,
    ISO2022RTL,
}

impl From<u8> for CharacterEncoding {
    fn from(value: u8) -> Self {
        match value {
            0x00 => CharacterEncoding::TRON8,
            0x10 => CharacterEncoding::PC,
            0x20 => CharacterEncoding::ISO2022,
            0x80 => CharacterEncoding::TRON8RTL,
            0xA0 => CharacterEncoding::ISO2022RTL,
            _ => CharacterEncoding::TRON8,
        }
    }
}

#[derive(Debug)]
struct Block {
    block_type: BlockType,
    attribute_data: String,
    body_data: String,
    plaintext: bool,
}

#[derive(Default)]
pub struct Scorpion {
    current_page_contents: Vec<Block>,
}

impl Scorpion {
    fn parse_body_data(&mut self, encoding: CharacterEncoding, body_data: &[u8]) -> String {
        let mut offset = 0;
        let mut body_string = String::new();

        while offset < body_data.len() {
            match body_data[offset] {
                // Whatever comes before it is some kind of section number or item number or a bullet indicating a list item.
                0x02 => {}
                // data+text sub-block start
                0x05 => {}
                // data+text sub-block separator
                0x06 => {}
                // data+text sub-block end
                0x07 => {}
                // Tab (preformatted only)
                0x09 => {}
                // Line break (preformatted only)
                0x0A => {}
                // Next byte - 0x40 is a graphics character from codepage 437
                0x10 => {
                    if encoding == CharacterEncoding::PC {
                        body_string.push(CP437_WINGDINGS.decode(body_data[offset + 1] - 0x40));
                        offset += 1;
                    }
                }
                // Normal style
                0x11 => {}
                // Strong style
                0x12 => {}
                // Emphasis style
                0x13 => {}
                // Monospace style
                0x14 => {}
                // Forward text direction
                0x15 => {}
                // Reverse text direction
                0x16 => {}
                // Furigana block main text
                0x17 => {}
                // Furigana block furigana text
                0x18 => {}
                // Furigana block end
                0x19 => {}
                // Used for SGR codes
                0x1B => {}
                // Only with ISO 2022 character encoding; must be immediately
                // followed by a GR character which is interpreted as G2 instead of G1
                // (further GR characters are interpreted as G1). (In PC and TRON encodings,
                // this code represents a graphic character or a part of one.)
                0x8E => {}
                // Like 0x8E but G3 instead of G2.
                0x8F => {}
                _ => {
                    if encoding == CharacterEncoding::PC {
                        body_string.push(CP437_CONTROL.decode(body_data[offset]));
                        //body_string.push(body_data[offset] as char);
                    } else {
                        body_string.push(body_data[offset] as char);
                    }
                }
            }
            offset += 1;
        }
        body_string
    }
}

impl ProtocolHandler for Scorpion {
    fn parse_content(&mut self, response: &[u8], plaintext: bool) {
        if plaintext {
            let block = Block {
                block_type: BlockType::Paragraph,
                attribute_data: String::new(),
                body_data: String::from_utf8_lossy(response).to_string(),
                plaintext: true,
            };
            self.current_page_contents = vec![block];
            return;
        }

        let mut blocks = Vec::new();
        let mut offset = 0;

        // We use 6 since it is the minimum possible block size (1 byte type/encoding, 2 bytes attribute length, 3 byte body length)
        while offset + 6 < response.len() {
            let block_type = BlockType::from(response[offset] & 0x0F);
            let character_encoding = CharacterEncoding::from(response[offset] & 0xF0);
            offset += 1;

            let attribute_length = (response[offset] as u16) << 8 | response[offset + 1] as u16;
            offset += 2;
            let attribute_data = response[offset..offset + attribute_length as usize].to_vec();
            offset += attribute_length as usize;

            let body_length = (response[offset] as u32) << 16
                | (response[offset + 1] as u32) << 8
                | (response[offset + 2] as u32);
            offset += 3;
            let body_data = response[offset..offset + body_length as usize].to_vec();
            offset += body_length as usize;

            blocks.push(Block {
                block_type,
                attribute_data: String::from_utf8_lossy(&attribute_data).to_string(),
                body_data: self.parse_body_data(character_encoding, &body_data),
                plaintext: false,
            });
        }

        self.current_page_contents = blocks;
    }

    fn render_page(&self, ui: &mut Ui, breeze: &Breeze) {
        self.current_page_contents
            .iter()
            .for_each(|block| match block.block_type {
                _ if block.plaintext => {
                    ui.monospace(&block.body_data);
                }
                BlockType::Paragraph => {
                    let text = RichText::new(&block.body_data).size(14.0);
                    ui.label(text);
                }
                BlockType::Heading1 => {
                    ui.label(RichText::new(&block.body_data).size(24.0));
                }
                BlockType::Heading2 => {
                    ui.label(RichText::new(&block.body_data).size(22.0));
                }
                BlockType::Heading3 => {
                    ui.label(RichText::new(&block.body_data).size(20.0));
                }
                BlockType::Heading4 => {
                    ui.label(RichText::new(&block.body_data).size(18.0));
                }
                BlockType::Heading5 => {
                    ui.label(RichText::new(&block.body_data).size(16.0));
                }
                BlockType::Heading6 => {
                    ui.label(RichText::new(&block.body_data).size(14.0));
                }
                BlockType::Hyperlink => {
                    let link_text = RichText::new(&block.body_data)
                        .color(Color32::BLUE)
                        .underline()
                        .monospace()
                        .size(14.0);
                    let current_url = breeze.current_url.clone();
                    let mut url = current_url.join(&block.attribute_data).unwrap().to_string();
                    if block.attribute_data.contains("://") {
                        url = block.attribute_data.clone();
                    }

                    let link = ui.add(Label::new(link_text).sense(Sense::hover()));
                    if link.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        *breeze.status_text.borrow_mut() = url.clone();
                    }
                    if link.clicked() {
                        breeze.url.set(url.clone());
                        let hint = if url.ends_with(".txt") {
                            Protocol::Plaintext
                        } else {
                            Protocol::from_url(&Url::parse(&url).unwrap())
                        };
                        breeze.navigation_hint.set(Some(NavigationHint {
                            url,
                            protocol: hint,
                            add_to_history: true,
                        }));
                    }
                }
                BlockType::Preformatted => {
                    let text = RichText::new(&block.body_data).size(14.0);
                    ui.code(text);
                }
                _ => {}
            });
    }
}
