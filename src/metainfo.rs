//! This module can be used to generate metainfo (.torrent) files, as specified in
//! [BEP 0003](https://www.bittorrent.org/beps/bep_0003.html) and
//! [BitTorrentSpecification](https://wiki.theory.org/index.php/BitTorrentSpecification)
use serde::Serialize;
use serde_bencode;

#[derive(Serialize)]
pub struct MetaInfo<'a> {
    announce: &'a str,
    info: InfoInner<'a>,
    // #[serde(rename = "announce-list")]
    // announce_list: Option<Vec<Vec<&'a str>>>,  // BEP-12
    // creation_date: Option<u64>,
    // comment: Option<&'a str>,
    // #[serde(rename = "created by")]
    // created_by: Option<&'a str>,
    // encoding: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum InfoInner<'a> {
    SingleFile {
        // filename
        name: &'a str,
        // number of bytes in each piece
        #[serde(rename = "piece length")]
        piece_length: u64,
        // bytestring consisting of the concatenation of all 20-byte SHA1 hash values, one per piece
        pieces: &'a str,
        // length of the file in bytes
        length: u64,
        // md5sum of the file
        md5sum: Option<&'a [u8; 32]>
    },
    MultipleFile {
        // directory name
        name: &'a str,
        // same as in SingleFile
        #[serde(rename = "piece length")]
        piece_length: u32,
        // same as in SingleFile
        pieces: &'a str,
        // list of files to distribute
        files: Vec<MetaInfoFile<'a>>,
    },
}

#[derive(Serialize)]
pub struct MetaInfoFile<'a> {
    // length of the file in bytes
    pub length: u64,
    // path to the file, each element is a directory except for the last, which is a filename
    pub path: Vec<&'a str>,
    pub md5sum: Option<&'a [u8; 32]>,
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
                md5sum: None,
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
