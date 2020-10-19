//! This module can be used to generate metainfo (.torrent) files, as specified in
//! [BEP 0003](https://www.bittorrent.org/beps/bep_0003.html).
use serde::Serialize;
use serde_bencode;

#[derive(Serialize)]
pub struct MetaInfo<'a> {
    announce: &'a str,
    info: InfoInner<'a>,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum InfoInner<'a> {
    SingleFile {
        name: &'a str,
        #[serde(rename = "piece length")]
        piece_length: u64,
        pieces: &'a str,
        length: u64,
    },
    MultipleFile {
        name: &'a str,
        #[serde(rename = "piece length")]
        piece_length: u32,
        pieces: &'a str,
        files: Vec<MetaInfoFile<'a>>,
    },
}

#[derive(Serialize)]
pub struct MetaInfoFile<'a> {
    pub length: u64,
    pub path: &'a str,
}

impl<'a> MetaInfo<'a> {
    pub fn bencode(&self) -> serde_bencode::Result<String> {
        serde_bencode::to_string(self)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basic_test() {
        let metainfo_single = MetaInfo {
            announce: "https://some_url",
            info: InfoInner::SingleFile {
                name: "filename",
                piece_length: 10,
                pieces: "abc",
                length: 100,
            },
        };
        // get this error when try to compare without unwrap():
        // error[E0369]: binary operation `==` cannot be applied to type `std::result::Result<std::string::String, serde_bencode::Error>`
        assert_eq!(
            metainfo_single.bencode().unwrap(),
            "d8:announce16:https://some_url4:infod6:lengthi100e4:name8:filename12:piece lengthi10e6:pieces3:abcee"
        );
    }
}
