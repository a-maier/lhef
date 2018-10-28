#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;
extern crate xmltree;
#[macro_use]
extern crate itertools;

pub mod fortran_blocks;
pub mod reader;
pub mod writer;
mod tags;

pub use fortran_blocks::*;
pub use reader::*;
pub use writer::*;

pub type XmlTree = xmltree::Element;

#[cfg(test)]
mod tests {
    extern crate flate2;

    use super::*;
    use std::io;
    use std::fs;
    use tests::flate2::bufread::GzDecoder;

    #[test]
    fn test_read_write() {
        let mut reader = {
            let file = fs::File::open(
                "test_data/2j.lhe.gz"
            ).expect("file not found");
            let reader = io::BufReader::new(
                GzDecoder::new(io::BufReader::new(file))
            );
            Reader::new(reader).unwrap()
        };
        let mut output = Vec::new();
        let mut events = Vec::new();
        {
            let mut writer = Writer::new(
                io::Cursor::new(&mut output), reader.version()
            ).unwrap();
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
