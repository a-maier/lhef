//! A library for interacting with files in the Les Houches Event File (LHEF) format.
//!
//! Lhef supports both reading and writing via the `Reader` and `Writer`
//! structs. Information about the generator run is provided in a `HEPRUP`
//! object and each event is stored in a `HEPEUP` object. These structs
//! correspond to the Fortran common blocks of the same names in the
//! [original proposal](https://arxiv.org/abs/hep-ph/0109068v1), but contain
//! extra `info` fields corresponding to the "optional information"
//! specified in the LHEF standard.
//!
//! As of now, only [version 1.0](https://arxiv.org/abs/hep-ph/0609017) of
//! the LHEF format</a> is fully supported. Files in [version
//! 2.0](http://www.lpthe.jussieu.fr/LesHouches09Wiki/index.php/LHEF_for_Matching)
//! and [3.0](https://phystev.cnrs.fr/wiki/2013:groups:tools:lhef3) are
//! parsed exactly like for version 1.0. This means that the additional XML
//! tags have to be extracted manually from the `info` fields of the
//! `HEPRUP` and `HEPEUP` objects.
//!
//! # Examples
//!
//! ```rust,no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use lhef::Reader;
//! use std::fs::File;
//! use std::io::BufReader;
//!
//! let input = BufReader::new(File::open("events.lhe")?);
//!
//! let mut reader = Reader::new(input)?;
//!
//! println!("Information in comment header:\n{}", reader.header());
//! println!("Information in xml header:\n{:?}", reader.xml_header());
//! println!("Generator run information:\n{:?}", reader.heprup());
//!
//! let event = reader.hepeup()?;
//! if let Some(event) = event {
//!     println!("Found an event: {:?}", event);
//! }
//! # Ok(())
//! # }
//! ```
mod data;
mod reader;
mod syntax;
mod writer;

pub use crate::data::HEPEUP as HEPEUP;
pub use crate::data::HEPRUP as HEPRUP;
pub use crate::data::XmlAttr as XmlAttr;
pub use crate::data::XmlTree as XmlTree;
pub use crate::reader::Reader as Reader;
pub use crate::writer::Writer as Writer;

#[cfg(test)]
mod tests {
    extern crate flate2;

    use super::*;
    use std::fs;
    use std::io;
    use tests::flate2::bufread::GzDecoder;

    #[test]
    fn test_read_write() {
        let mut reader = {
            let file =
                fs::File::open("test_data/2j.lhe.gz").expect("file not found");
            let reader =
                io::BufReader::new(GzDecoder::new(io::BufReader::new(file)));
            Reader::new(reader).unwrap()
        };
        let mut output = Vec::new();
        let mut events = Vec::new();
        {
            let mut writer =
                Writer::new(io::Cursor::new(&mut output), reader.version())
                    .unwrap();
            if let Some(header) = reader.xml_header() {
                writer.xml_header(header).unwrap();
            }
            if !reader.header().is_empty() {
                writer.header(reader.header()).unwrap();
            }
            writer.heprup(reader.heprup()).unwrap();
            while let Some(event) = reader.hepeup().unwrap() {
                writer.hepeup(&event).unwrap();
                events.push(event);
            }
            writer.finish().unwrap();
        }
        let mut cmp_reader = Reader::new(io::Cursor::new(&output)).unwrap();
        assert_eq!(cmp_reader.version(), reader.version());
        assert_eq!(cmp_reader.header(), reader.header());
        assert_eq!(cmp_reader.xml_header(), reader.xml_header());
        assert_eq!(cmp_reader.heprup(), reader.heprup());
        let mut cmp_events = Vec::new();
        while let Some(event) = cmp_reader.hepeup().unwrap() {
            cmp_events.push(event);
        }
        assert_eq!(cmp_events, events)
    }
}
