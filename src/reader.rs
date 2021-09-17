use crate::data::*;
use crate::syntax::*;

use std::io::BufRead;
use std::str;

use thiserror::Error;

/// Reader for the LHEF format
#[derive(Debug, PartialEq)]
pub struct Reader<T> {
    stream: T,
    version: &'static str,
    header: String,
    xml_header: Option<XmlTree>,
    heprup: HEPRUP,
}

impl<T: BufRead> Reader<T> {
    /// Create a new LHEF reader
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let file = std::fs::File::open("events.lhe").unwrap();
    /// let file = std::io::BufReader::new(file);
    /// let reader = lhef::Reader::new(file).unwrap();
    /// ```
    pub fn new(mut stream: T) -> Result<Reader<T>, ReadError> {
        let version = parse_version(&mut stream)?;
        let (header, xml_header, init_start) = parse_header(&mut stream)?;
        let heprup = parse_init(&init_start, &mut stream)?;
        Ok(Reader {
            stream,
            version,
            header,
            xml_header,
            heprup,
        })
    }

    /// Get the LHEF version
    pub fn version(&self) -> &str {
        self.version
    }

    /// Get the LHEF header
    pub fn header(&self) -> &str {
        &self.header
    }

    /// Get the LHEF xml header
    pub fn xml_header(&self) -> &Option<XmlTree> {
        &self.xml_header
    }

    /// Get the run information in HEPRUP format
    pub fn heprup(&self) -> &HEPRUP {
        &self.heprup
    }

    /// Get the next event in HEPEUP format
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let file = std::fs::File::open("events.lhe").unwrap();
    /// let file = std::io::BufReader::new(file);
    /// let mut reader = lhef::Reader::new(file).unwrap();
    ///
    /// let event = reader.hepeup().unwrap();
    /// match event {
    ///    Some(event) => println!("Found an event."),
    ///    None => println!("Reached end of event file."),
    /// }
    /// ```
    pub fn hepeup(&mut self) -> Result<Option<HEPEUP>, ReadError> {
        let mut line = String::new();
        self.stream.read_line(&mut line)?;
        if line.starts_with(EVENT_START) {
            Ok(Some(parse_event(&line, &mut self.stream)?))
        } else if line.trim() == LHEF_LAST_LINE {
            Ok(None)
        } else {
            Err(ReadError::BadEventStart(line))
        }
    }
}

fn parse_version<T: BufRead>(
    stream: &mut T,
) -> Result<&'static str, ReadError> {
    use self::ReadError::*;
    let mut first_line = String::new();
    stream.read_line(&mut first_line)?;
    let line_cp = first_line.clone();
    let mut line_entries = first_line.trim().split('"');
    if line_entries.next() != Some(LHEF_TAG_OPEN) {
        return Err(ReadError::BadFirstLine(line_cp));
    };
    let version = match line_entries.next() {
        Some("1.0") => "1.0",
        Some("2.0") => "2.0",
        Some("3.0") => "3.0",
        Some(version) => {
            return Err(UnsupportedVersion(version.to_string()))
        }
        None => return Err(MissingVersion),
    };
    if line_entries.next() != Some(">") {
        return Err(ReadError::BadFirstLine(line_cp));
    };
    Ok(version)
}

fn parse_header<T: BufRead>(
    mut stream: &mut T,
) -> Result<(String, Option<XmlTree>, String), ReadError> {
    use ReadError::BadHeaderStart;
    let mut header = String::new();
    let mut xml_header = None;
    loop {
        let mut header_text = String::new();
        stream.read_line(&mut header_text)?;
        if header_text.trim_start().starts_with(COMMENT_START) {
            if header_text.trim() != COMMENT_START {
                return Err(BadHeaderStart(header_text));
            }
            read_lines_until(&mut stream, &mut header_text, COMMENT_END)?;
            header = header_text;
        } else if header_text.trim_start().starts_with(HEADER_START) {
            read_lines_until(&mut stream, &mut header_text, HEADER_END)?;
            xml_header = Some(XmlTree::parse(header_text.as_bytes())?);
        } else if header_text.trim_start().starts_with(INIT_START) {
            return Ok((header, xml_header, header_text));
        } else {
            return Err(ReadError::BadHeaderStart(header_text));
        }
    }
}

fn pop_line(s: &mut String) {
    s.pop();
    while !s.is_empty() && !s.ends_with('\n') {
        s.pop();
    }
}

fn read_lines_until<T: BufRead>(
    stream: &mut T,
    header: &mut String,
    header_end: &str,
) -> Result<(), ReadError> {
    loop {
        if stream.read_line(header)? == 0 {
            return Err(ReadError::EndOfFile("header"));
        }
        if header.lines().last().unwrap().trim() == header_end {
            return Ok(());
        }
    }
}

fn parse<F, T, S>(name: F, text: Option<&str>) -> Result<T, ReadError>
where
    T: str::FromStr,
    F: FnOnce() ->  S,
    S: Into<String>
{
    use self::ReadError::*;
    let text: &str = text.ok_or_else(
        || MissingEntry(name().into())
    )?;
    match text.parse::<T>() {
        Ok(t) => Ok(t),
        Err(_) => Err(ConversionError(text.to_owned())),
    }
}

fn parse_f64<F, S>(name: F, text: Option<&str>) -> Result<f64, ReadError>
where
    F: FnOnce() ->  S,
    S: Into<String>
{
    use self::ReadError::*;
    let text: &str = text.ok_or_else(
        || MissingEntry(name().into())
    )?;
    match fast_float::parse(text) {
        Ok(t) => Ok(t),
        Err(_) => Err(ConversionError(text.to_owned())),
    }
}

fn extract_xml_attr_str(xml_tag: &str) -> Result<&str, ReadError> {
    use self::ReadError::BadXmlTag;
    let tag = xml_tag.trim();
    if !tag.ends_with('>') {
        return Err(BadXmlTag(xml_tag.to_owned()));
    }
    let len = tag.len();
    let tag = &tag[..len - 1];
    let first_attr = tag.find(char::is_whitespace);
    let tag = match first_attr {
        None => return Ok(""),
        Some(idx) => &tag[idx + 1..],
    };
    Ok(tag.trim_start())
}

struct Attr<'a> {
    name: &'a str,
    value: &'a str,
}

fn next_attr(
    attr_str: &str,
) -> Result<(Option<Attr>, &str), ReadError> {
    use self::ReadError::BadXmlTag;
    let mut rem = attr_str;
    let name_end = rem.find(|c: char| c.is_whitespace() || c == '=');
    let name = match name_end {
        None => return Ok((None, rem)),
        Some(idx) => &rem[..idx],
    };
    rem = rem[name.len()..].trim_start();
    if !rem.starts_with('=') {
        return Err(BadXmlTag(attr_str.to_owned()));
    }
    rem = rem[1..].trim_start();
    let quote = rem.chars().next();
    if quote != Some('\'') && quote != Some('"') {
        return Err(BadXmlTag(attr_str.to_owned()));
    }
    let quote = quote.unwrap();
    rem = &rem[1..];
    let value_end = rem.find(quote);
    let value = match value_end {
        Some(idx) => &rem[..idx],
        None => return Err(BadXmlTag(attr_str.to_owned())),
    };
    rem = rem[value.len() + 1..].trim_start();
    let attr = Attr { name, value };
    Ok((Some(attr), rem))
}

fn extract_xml_attr(xml_tag: &str) -> Result<XmlAttr, ReadError> {
    let mut attr_str = extract_xml_attr_str(xml_tag)?;
    let mut attr = XmlAttr::new();
    loop {
        let (parsed, rem) = next_attr(attr_str)?;
        match parsed {
            None => return Ok(attr),
            Some(next_attr) => {
                let name = next_attr.name.to_string();
                let value = next_attr.value.to_string();
                attr.insert(name, value);
            }
        };
        attr_str = rem;
    }
}

#[allow(non_snake_case)]
fn parse_init<T: BufRead>(
    init_open: &str,
    stream: &mut T,
) -> Result<HEPRUP, ReadError> {
    let mut line = String::new();
    stream.read_line(&mut line)?;
    let mut entries = line.split_whitespace();
    let IDBMUP = [
        parse(|| "IDBMUP(1)", entries.next())?,
        parse(|| "IDBMUP(2)", entries.next())?,
    ];
    let EBMUP = [
        parse_f64(|| "EBMUP(1)", entries.next())?,
        parse_f64(|| "EBMUP(2)", entries.next())?,
    ];
    let PDFGUP = [
        parse(|| "PDFGUP(1)", entries.next())?,
        parse(|| "PDFGUP(2)", entries.next())?,
    ];
    let PDFSUP = [
        parse(|| "PDFSUP(1)", entries.next())?,
        parse(|| "PDFSUP(2)", entries.next())?,
    ];
    let IDWTUP = parse(|| "IDWTUP", entries.next())?;
    let NPRUP = parse(|| "NPRUP", entries.next())?;
    let mut XSECUP = Vec::with_capacity(NPRUP as usize);
    let mut XERRUP = Vec::with_capacity(NPRUP as usize);
    let mut XMAXUP = Vec::with_capacity(NPRUP as usize);
    let mut LPRUP = Vec::with_capacity(NPRUP as usize);
    for i in 0..NPRUP {
        let mut line = String::new();
        stream.read_line(&mut line)?;
        let mut entries = line.split_whitespace();
        XSECUP
            .push(parse_f64(|| format!("XSECUP({})", i + 1), entries.next())?);
        XERRUP
            .push(parse_f64(|| format!("XERRUP({})", i + 1), entries.next())?);
        XMAXUP
            .push(parse_f64(|| format!("XMAXUP({})", i + 1), entries.next())?);
        LPRUP.push(parse(|| format!("LPRUP({})", i + 1), entries.next())?);
    }
    let mut info = String::new();
    loop {
        if stream.read_line(&mut info)? == 0 {
            return Err(ReadError::EndOfFile("init"));
        }
        if info.lines().last().unwrap() == INIT_END {
            pop_line(&mut info);
            break;
        }
    }
    let attr = extract_xml_attr(init_open)?;
    Ok(HEPRUP {
        IDBMUP,
        EBMUP,
        PDFGUP,
        PDFSUP,
        IDWTUP,
        NPRUP,
        XSECUP,
        XERRUP,
        XMAXUP,
        LPRUP,
        info,
        attr,
    })
}

#[allow(non_snake_case)]
fn parse_event<T: BufRead>(
    event_open: &str,
    stream: &mut T,
) -> Result<HEPEUP, ReadError> {
    let mut line = String::new();
    stream.read_line(&mut line)?;
    let mut entries = line.split_whitespace();
    let NUP = parse(|| "NUP", entries.next())?;
    let IDRUP = parse(|| "IDRUP", entries.next())?;
    let XWGTUP = parse_f64(|| "XWGTUP", entries.next())?;
    let SCALUP = parse_f64(|| "SCALUP", entries.next())?;
    let AQEDUP = parse_f64(|| "AQEDUP", entries.next())?;
    let AQCDUP = parse_f64(|| "AQCDUP", entries.next())?;
    let mut IDUP = Vec::with_capacity(NUP as usize);
    let mut ISTUP = Vec::with_capacity(NUP as usize);
    let mut MOTHUP = Vec::with_capacity(NUP as usize);
    let mut ICOLUP = Vec::with_capacity(NUP as usize);
    let mut PUP = Vec::with_capacity(NUP as usize);
    let mut VTIMUP = Vec::with_capacity(NUP as usize);
    let mut SPINUP = Vec::with_capacity(NUP as usize);
    for i in 0..NUP {
        let mut line = String::new();
        stream.read_line(&mut line)?;
        let mut entries = line.split_whitespace();
        IDUP.push(parse(|| format!("IDUP({})", i + 1), entries.next())?);
        ISTUP.push(parse(|| format!("ISTUP({})", i + 1), entries.next())?);
        MOTHUP.push([
            parse(|| format!("MOTHUP({}, 1)", i + 1), entries.next())?,
            parse(|| format!("MOTHUP({}, 2)", i + 1), entries.next())?,
        ]);
        ICOLUP.push([
            parse(|| format!("ICOLUP({}, 1)", i + 1), entries.next())?,
            parse(|| format!("ICOLUP({}, 2)", i + 1), entries.next())?,
        ]);
        PUP.push([
            parse_f64(|| format!("PUP({}, 1)", i + 1), entries.next())?,
            parse_f64(|| format!("PUP({}, 2)", i + 1), entries.next())?,
            parse_f64(|| format!("PUP({}, 3)", i + 1), entries.next())?,
            parse_f64(|| format!("PUP({}, 4)", i + 1), entries.next())?,
            parse_f64(|| format!("PUP({}, 5)", i + 1), entries.next())?,
        ]);
        VTIMUP
            .push(parse_f64(|| format!("VTIMUP({})", i + 1), entries.next())?);
        SPINUP
            .push(parse_f64(|| format!("SPINUP({})", i + 1), entries.next())?);
    }
    let mut info = String::new();
    loop {
        if stream.read_line(&mut info)? == 0 {
            return Err(ReadError::EndOfFile("event"));
        }
        if info.lines().last().unwrap().trim() == EVENT_END {
            pop_line(&mut info);
            break;
        }
    }
    let attr = extract_xml_attr(event_open)?;
    Ok(HEPEUP {
        NUP,
        IDRUP,
        XWGTUP,
        SCALUP,
        AQEDUP,
        AQCDUP,
        IDUP,
        ISTUP,
        MOTHUP,
        ICOLUP,
        PUP,
        VTIMUP,
        SPINUP,
        info,
        attr,
    })
}

#[derive(Error, Debug)]
pub enum ReadError {
    #[error("First line '{0}' in input does start with '{}'", LHEF_TAG_OPEN)]
    BadFirstLine(String),
    #[error(
        "Encountered unrecognized line '{0}', \
         expected a header starting with '{}', '{}', \
         or the init block starting with '{}'",
        COMMENT_START, HEADER_START, INIT_START
    )]
    BadHeaderStart(String),
    #[error("Encountered malformed xml tag: '{0}'")]
    BadXmlTag(String),
    #[error(
        "Encountered unrecognized line '{0}', \
         expected an event starting with '{}'",
        EVENT_START
    )]
    BadEventStart(String),
    #[error("Missing entry '{0}'")]
    MissingEntry(String),
    #[error("Failed to convert to number: '{0}'")]
    ConversionError(String),
    #[error("Unsupported version '{0}': only '1.0', '2.0', '3.0' are supported")]
    UnsupportedVersion(String),
    #[error("Version information missing")]
    MissingVersion,
    #[error("Encountered '{0}' block without closing tag")]
    EndOfFile(&'static str),
    #[error("Read error: {0}")]
    ReadErr(#[from] std::io::Error),
    #[error("xml parse error: {0}")]
    XmlErr(#[from] xmltree::ParseError),
}

#[cfg(test)]
mod reader_tests {
    extern crate flate2;
    use super::*;

    use reader_tests::flate2::bufread::GzDecoder;
    use std::fs::File;
    use std::io::BufReader;

    #[test]
    fn read_correct() {
        let file = File::open("test_data/2j.lhe.gz").expect("file not found");
        let reader = BufReader::new(GzDecoder::new(BufReader::new(file)));
        let mut lhef = Reader::new(reader).unwrap();
        assert_eq!(lhef.version(), "3.0");
        {
            let header = lhef.xml_header().as_ref().unwrap();
            let mg_version_entry = &header.children[0];
            assert_eq!(mg_version_entry.name, "MGVersion");
            assert_eq!(mg_version_entry.text.as_ref().unwrap(), "\n#5.2.3.3\n");
        }
        assert!(lhef.heprup().attr.is_empty());
        let mut nevents = 0;
        while let Ok(Some(_)) = lhef.hepeup() {
            nevents += 1
        }
        assert_eq!(nevents, 1628);
    }

    #[test]
    fn read_hejfog() {
        let file =
            File::open("test_data/HEJFOG.lhe.gz").expect("file not found");
        let reader = BufReader::new(GzDecoder::new(BufReader::new(file)));
        let mut lhef = Reader::new(reader).unwrap();
        {
            let attr = lhef.heprup().attr.get("testattribute");
            assert_eq!(attr.unwrap().as_str(), "testvalue");
        }
        let first_event = lhef.hepeup().unwrap().unwrap();
        let expected_attr = {
            let mut hash = XmlAttr::new();
            hash.insert(String::from("attr0"), String::from("t0"));
            hash.insert(String::from("attr1"), String::from(""));
            hash
        };
        assert_eq!(first_event.attr, expected_attr);
        let mut nevents = 1;
        while let Ok(Some(_)) = lhef.hepeup() {
            nevents += 1
        }
        assert_eq!(nevents, 10);
    }
}
