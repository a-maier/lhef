#![doc(html_root_url = "https://docs.rs/lhef/0.2.0")]
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;
extern crate xmltree;
#[macro_use]
extern crate itertools;

use std::fmt;
use std::fmt::Write as FmtWrite;
use std::str;
use std::error;
use std::io::{BufRead,Write};
use std::collections::HashMap;
use std::ops::Drop;

pub type XmlTree = xmltree::Element;

const LHEF_TAG_OPEN: &'static str = "<LesHouchesEvents version=";
const COMMENT_START: &'static str = "<!--";
const COMMENT_END: &'static str = "-->";
const HEADER_START: &'static str = "<header";
const HEADER_END: &'static str = "</header>";
const INIT_START: &'static str = "<init";
const INIT_END: &'static str = "</init>";
const EVENT_START: &'static str = "<event";
const EVENT_END: &'static str = "</event>";
const LHEF_LAST_LINE: &'static str = "</LesHouchesEvents>";

pub type XmlAttr = HashMap<String, String>;

/// Generator run information
///
/// See <https://arxiv.org/abs/hep-ph/0109068v1> for details on the fields.
#[allow(non_snake_case)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(PartialEq,Debug,Clone)]
pub struct HEPRUP {
    /// Beam IDs
    pub IDBMUP: [i32; 2],
    /// Beam energies
    pub EBMUP: [f64; 2],
    /// PDF groups
    pub PDFGUP: [i32; 2],
    /// PDF set IDs
    pub PDFSUP: [i32; 2],
    /// Event weight specification
    pub IDWTUP: i32,
    /// Number of subprocesses
    pub NPRUP: i32,
    /// Subprocess cross sections
    pub XSECUP: Vec<f64>,
    /// Subprocess cross section errors
    pub XERRUP: Vec<f64>,
    /// Subprocess maximum weights
    pub XMAXUP: Vec<f64>,
    /// Process IDs
    pub LPRUP: Vec<i32>,
    /// Optional run information
    pub info: String,
    /// Attributes in <init> tag
    pub attr: XmlAttr,
}

/// Event information
///
/// See <https://arxiv.org/abs/hep-ph/0109068v1> for details on the fields.
#[allow(non_snake_case)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(PartialEq,Debug,Clone)]
pub struct HEPEUP{
    /// Number of particles
    pub NUP: i32,
    /// Process ID
    pub IDRUP: i32,
    /// Event weight
    pub XWGTUP: f64,
    /// Scale in GeV
    pub SCALUP: f64,
    /// Value of the QED coupling α
    pub AQEDUP: f64,
    /// Value of the QCD coupling α_s
    pub AQCDUP: f64,
    /// Particle IDs
    pub IDUP: Vec<i32>,
    /// Particle status
    pub ISTUP: Vec<i32>,
    /// Indices of decay mothers
    pub MOTHUP: Vec<[i32; 2]>,
    /// Colour flow
    pub ICOLUP: Vec<[i32; 2]>,
    /// Particle momentum in GeV
    pub PUP: Vec<[f64; 5]>,
    /// Lifetime in mm
    pub VTIMUP: Vec<f64>,
    /// Spin angle
    pub SPINUP: Vec<f64>,
    /// Optional event information
    pub info: String,
    /// Attributes in <event> tag
    pub attr: XmlAttr,
}

/// Reader for the LHEF format
#[derive(Debug,PartialEq)]
pub struct Reader<T> {
    stream: T,
    version: &'static str,
    header: String,
    xml_header: Option<XmlTree>,
    heprup: HEPRUP,
}

#[derive(Debug,PartialEq,Clone,Eq,Hash)]
enum ParseError {
    BadFirstLine(String),
    BadHeaderStart(String),
    BadXmlTag(String),
    BadEventStart(String),
    MissingEntry(String),
    ConversionError(String),
    UnsupportedVersion(String),
    MissingVersion,
    EndOfFile(&'static str),
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
    pub fn new(mut stream: T) -> Result<Reader<T>, Box<error::Error>> {
        let version = parse_version(&mut stream)?;
        let (header, xml_header, init_start) = parse_header(&mut stream)?;
        let heprup = parse_init(&init_start, &mut stream)?;
        Ok(Reader{stream, version, header, xml_header, heprup})
    }

    /// Get the LHEF version
    pub fn version(&self) -> &str {
        &self.version
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
    pub fn hepeup(&mut self) -> Result<Option<HEPEUP>, Box<error::Error>> {
        let mut line = String::new();
        self.stream.read_line(&mut line)?;
        if line.starts_with(EVENT_START) {
            Ok(Some(parse_event(&line, &mut self.stream)?))
        }
        else if line.trim() == LHEF_LAST_LINE {
            Ok(None)
        }
        else {
            Err(Box::new(ParseError::BadEventStart(line)))
        }
    }
}

fn parse_version<T: BufRead>(stream: &mut T) -> Result<&'static str, Box<error::Error>> {
    use self::ParseError::*;
    let mut first_line = String::new();
    stream.read_line(&mut first_line)?;
    let line_cp = first_line.clone();
    let mut line_entries = first_line.trim().split('"');
    if line_entries.next() != Some(LHEF_TAG_OPEN) {
        return Err(Box::new(ParseError::BadFirstLine(line_cp)))
    };
    let version = match line_entries.next() {
        Some("1.0") => {"1.0"},
        Some("2.0") => {"2.0"},
        Some("3.0") => {"3.0"},
        Some(version) => {
            return Err(Box::new(UnsupportedVersion(version.to_string())))
        }
        None => {
            return Err(Box::new(MissingVersion))
        }
    };
    if line_entries.next() != Some(">") {
        return Err(Box::new(ParseError::BadFirstLine(line_cp)))
    };
    Ok(version)
}

fn parse_header<T: BufRead>(mut stream: &mut T) ->
    Result<(String, Option<XmlTree>, String), Box<error::Error>>
{
    use self::ParseError::BadHeaderStart;
    let mut header = String::new();
    let mut xml_header = None;
    loop {
        let mut header_text = String::new();
        stream.read_line(&mut header_text)?;
        if header_text.trim_left().starts_with(COMMENT_START) {
            if header_text.trim() != COMMENT_START {
                return Err(Box::new(BadHeaderStart(header_text)))
            }
            read_lines_until(&mut stream, &mut header_text, COMMENT_END)?;
            header = header_text;
        }
        else if header_text.trim_left().starts_with(HEADER_START) {
            read_lines_until(&mut stream, &mut header_text, HEADER_END)?;
            xml_header = Some(XmlTree::parse(header_text.as_bytes())?);
        }
        else if header_text.trim_left().starts_with(INIT_START) {
            return Ok((header, xml_header, header_text))
        }
        else {
            return Err(Box::new(ParseError::BadHeaderStart(header_text)))
        }
    }
}

fn pop_line(s: &mut String) {
    s.pop();
    while !s.is_empty() && s.chars().last().unwrap() != '\n' {
        s.pop();
    }
}

fn read_lines_until<T: BufRead>(
    stream: &mut T, header: &mut String, header_end: &str
) -> Result<(), Box<error::Error>> {
    loop {
        if stream.read_line(header)? == 0 {
            return Err(Box::new(ParseError::EndOfFile("header")));
        }
        if header.lines().last().unwrap().trim() == header_end {
            return Ok(())
        }
    }
}

fn parse<T>(name: &str, text: Option<&str>) -> Result<T, Box<error::Error>>
where T: str::FromStr {
    use self::ParseError::*;
    let text: &str = text.ok_or(Box::new(MissingEntry(String::from(name))))?;
    match text.parse::<T>() {
        Ok(t) => Ok(t),
        Err(_) => Err(Box::new(ConversionError(text.to_owned())))
    }
}

fn extract_xml_attr_str(xml_tag: &str) -> Result<&str, Box<error::Error>> {
    use self::ParseError::BadXmlTag;
    let tag = xml_tag.trim();
    if tag.chars().last() != Some('>') {
        return Err(Box::new(BadXmlTag(xml_tag.to_owned())));
    }
    let len = tag.len();
    let tag = &tag[..len-1];
    let first_attr = tag.find(char::is_whitespace);
    let tag = match first_attr {
        None => return Ok(""),
        Some(idx) => &tag[idx+1..],
    };
    Ok(tag.trim_left())
}

struct Attr<'a> {
    name: &'a str,
    value: &'a str,
}

fn next_attr(attr_str: &str) -> Result<(Option<Attr>, &str), Box<error::Error>> {
    use self::ParseError::BadXmlTag;
    let mut rem = attr_str;
    let name_end = rem.find(|c: char| c.is_whitespace() || c == '=');
    let name = match name_end {
        None => return Ok((None, rem)),
        Some(idx) => &rem[..idx],
    };
    rem = rem[name.len()..].trim_left();
    if rem.chars().next() != Some('=') {
        return Err(Box::new(BadXmlTag(attr_str.to_owned())));
    }
    rem = rem[1..].trim_left();
    let quote = rem.chars().next();
    if quote != Some('\'') && quote != Some('"') {
        return Err(Box::new(BadXmlTag(attr_str.to_owned())));
    }
    let quote = quote.unwrap();
    rem = &rem[1..];
    let value_end = rem.find(quote);
    let value = match value_end {
        Some(idx) => &rem[..idx],
        None => return Err(Box::new(BadXmlTag(attr_str.to_owned()))),
    };
    rem = &rem[value.len()+1..].trim_left();
    let attr = Attr{name, value};
    Ok((Some(attr), rem))
}

fn extract_xml_attr(xml_tag: &str) -> Result<XmlAttr, Box<error::Error>> {
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
                },
            };
            attr_str = rem;
        }
}

#[allow(non_snake_case)]
fn parse_init<T: BufRead>(
    init_open: &str, stream: &mut T
) -> Result<HEPRUP, Box<error::Error>> {
    let mut line = String::new();
    stream.read_line(&mut line)?;
    let mut entries = line.split_whitespace();
    let IDBMUP = [
        parse::<i32>("IDBMUP(1)", entries.next())?,
        parse::<i32>("IDBMUP(2)", entries.next())?,
    ];
    let EBMUP = [
        parse::<f64>("EBMUP(1)", entries.next())?,
        parse::<f64>("EBMUP(2)", entries.next())?,
    ];
    let PDFGUP = [
        parse::<i32>("PDFGUP(1)", entries.next())?,
        parse::<i32>("PDFGUP(2)", entries.next())?,
    ];
    let PDFSUP = [
        parse::<i32>("PDFSUP(1)", entries.next())?,
        parse::<i32>("PDFSUP(2)", entries.next())?,
    ];
    let IDWTUP = parse::<i32>("IDWTUP", entries.next())?;
    let NPRUP = parse::<i32>("NPRUP", entries.next())?;
    let mut XSECUP = Vec::with_capacity(NPRUP as usize);
    let mut XERRUP = Vec::with_capacity(NPRUP as usize);
    let mut XMAXUP = Vec::with_capacity(NPRUP as usize);
    let mut LPRUP = Vec::with_capacity(NPRUP as usize);
    for i in 0..NPRUP {
        let mut line = String::new();
        stream.read_line(&mut line)?;
        let mut entries = line.split_whitespace();
        XSECUP.push(parse::<f64>(&format!("XSECUP({})", i+1), entries.next())?);
        XERRUP.push(parse::<f64>(&format!("XERRUP({})", i+1), entries.next())?);
        XMAXUP.push(parse::<f64>(&format!("XMAXUP({})", i+1), entries.next())?);
        LPRUP.push(parse::<i32> (&format!("LPRUP({})", i+1), entries.next())?);
    }
    let mut info = String::new();
    loop {
        if stream.read_line(&mut info)? == 0 {
            return Err(Box::new(ParseError::EndOfFile("init")));
        }
        if info.lines().last().unwrap() == INIT_END {
            pop_line(&mut info);
            break;
        }
    }
    let attr = extract_xml_attr(init_open)?;
    Ok(HEPRUP{
        IDBMUP, EBMUP, PDFGUP, PDFSUP, IDWTUP, NPRUP,
        XSECUP, XERRUP, XMAXUP, LPRUP,
        info, attr
    })
}

#[allow(non_snake_case)]
fn parse_event<T: BufRead>(
    event_open: &str, stream: &mut T
) -> Result<HEPEUP, Box<error::Error>> {
    let mut line = String::new();
    stream.read_line(&mut line)?;
    let mut entries = line.split_whitespace();
    let NUP = parse::<i32>("NUP", entries.next())?;
    let IDRUP = parse::<i32>("IDRUP", entries.next())?;
    let XWGTUP = parse::<f64>("XWGTUP", entries.next())?;
    let SCALUP = parse::<f64>("SCALUP", entries.next())?;
    let AQEDUP = parse::<f64>("AQEDUP", entries.next())?;
    let AQCDUP = parse::<f64>("AQCDUP", entries.next())?;
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
        IDUP.push(parse::<i32>(&format!("IDUP({})", i+1), entries.next())?);
        ISTUP.push(parse::<i32>(&format!("ISTUP({})", i+1), entries.next())?);
        MOTHUP.push([
            parse::<i32>(&format!("MOTHUP({}, 1)", i+1), entries.next())?,
            parse::<i32>(&format!("MOTHUP({}, 2)", i+1), entries.next())?,
        ]);
        ICOLUP.push([
            parse::<i32>(&format!("ICOLUP({}, 1)", i+1), entries.next())?,
            parse::<i32>(&format!("ICOLUP({}, 2)", i+1), entries.next())?,
        ]);
        PUP.push([
            parse::<f64>(&format!("PUP({}, 1)", i+1), entries.next())?,
            parse::<f64>(&format!("PUP({}, 2)", i+1), entries.next())?,
            parse::<f64>(&format!("PUP({}, 3)", i+1), entries.next())?,
            parse::<f64>(&format!("PUP({}, 4)", i+1), entries.next())?,
            parse::<f64>(&format!("PUP({}, 5)", i+1), entries.next())?,
        ]);
        VTIMUP.push(parse::<f64>(&format!("VTIMUP({})", i+1), entries.next())?);
        SPINUP.push(parse::<f64>(&format!("SPINUP({})", i+1), entries.next())?);
    }
    let mut info = String::new();
    loop {
        if stream.read_line(&mut info)? == 0 {
            return Err(Box::new(ParseError::EndOfFile("event")));
        }
        if info.lines().last().unwrap().trim() == EVENT_END {
            pop_line(&mut info);
            break;
        }
    }
    let attr = extract_xml_attr(event_open)?;
    Ok(HEPEUP{
        NUP, IDRUP, XWGTUP, SCALUP, AQEDUP, AQCDUP,
        IDUP, ISTUP, MOTHUP, ICOLUP, PUP, VTIMUP, SPINUP,
        info, attr,
    })
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ParseError::*;
        match *self {
            BadFirstLine(ref line) => {
                write!(
                    f,
                    "First line '{}' in input does start with '{}'",
                    line, LHEF_TAG_OPEN
                )
            },
            BadHeaderStart(ref line) => {
                write!(
                    f,
                    "Encountered unrecognized line '{}', \
                     expected a header starting with '{}', '{}', \
                     or the init block starting with '{}'",
                    line, COMMENT_START, HEADER_START, INIT_START
                )
            },
            BadXmlTag(ref line) => {
                write!(
                    f,
                    "Encountered malformed xml tag: '{}'",
                    line
                )
            },
            BadEventStart(ref line) => {
                write!(
                    f,
                    "Encountered unrecognized line '{}', \
                     expected an event starting with '{}'",
                    line, EVENT_START
                )
            },
            UnsupportedVersion(ref version) => {
                write!(
                    f,
                    "Unsupported version {}, only 1.0, 2.0, 3.0 are supported",
                    version
                )
            },
            MissingVersion => {
                write!(f, "Version information missing")
            }
            MissingEntry(ref entry) => {
                write!(f, "Missing entry '{}'", entry)
            },
            ConversionError(ref entry) => {
                write!(f, "Failed to convert to number: '{}'", entry)
            },
            EndOfFile(ref block) => {
                write!(f, "Encountered '{}' block without closing tag", block)
            }
        }
    }
}

// TODO
impl error::Error for ParseError {
    fn description(&self) -> &str {
        ""
    }

    fn cause(&self) -> Option<&error::Error> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

fn xml_to_string(xml: &XmlTree, output: &mut String) {
    *output += "<";
    *output += &xml.name;
    for (key, value) in &xml.attributes {
        *output += &format!(" {}=\"{}\"", key, value);
    }
    *output += ">";
    if let Some(ref text) = xml.text {
        *output += text;
    }
    for child in &xml.children {
        xml_to_string(&child, output)
    }
    *output += &format!("</{}>", xml.name);
}

/// Writer for the LHEF format
///
/// The general usage to write a file is
///
/// ```rust,ignore
/// // create writer
/// let mut writer = lhef::Writer::new(my_file, version)?;
/// // optionally write headers
/// writer.header(my_header)?;
/// writer.xml_header(my_xml_header)?;
/// // write run information
/// writer.heprup(my_heprup)?;
/// // write events
/// loop {
///    writer.hepeup(my_hepeup)?;
/// }
/// //  wrap up
/// writer.finish()?;
/// ```
/// It is important to keep the proper order of method calls and to call
/// finish() at the end.
#[derive(Debug,PartialEq,Eq)]
pub struct Writer<T: Write> {
    stream: T,
    state: WriterState,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug,Clone,Hash,PartialEq,Eq,Copy)]
// State of LHEF writer
enum WriterState {
    // The next object to be written should be a header or an init block
    ExpectingHeaderOrInit,
    // The writer can either write an event or finish the LHEF file
    ExpectingEventOrFinish,
    // The LHEF file is complete and no further writing is allowed
    Finished,
    // A previous write failed and the LHEF file is in an undetermined
    // (possible broken) state
    Failed,
}

#[derive(Debug,Clone,Hash,PartialEq,Eq)]
enum WriteError {
    MismatchedSubprocesses,
    MismatchedParticles,
    BadState(WriterState, &'static str),
    WriteToFailed,
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::WriteError::*;
        match *self {
            MismatchedSubprocesses => {
                write!(
                    f,
                    "Mismatch between NPRUP and length of at least one of \
                     XSECUP, XERRUP, XMAXUP, LPRUP."
                )
            },
            MismatchedParticles => {
                write!(
                    f,
                    "Mismatch between NUP and length of at least one of \
                     IDUP, ISTUP, MOTHUP, ICOLUP, PUP, VTIMUP, SPINUP."
                )
            },
            BadState(ref state, attempt) => {
                write!(
                    f,
                    "Writer is in state '{:?}', cannot write '{}'.",
                    state, attempt
                )
            },
            WriteToFailed => {
                write!(
                    f,
                    "Writer is in 'Failed' state. \
                     Output was written, but the file may be broken anyway."
                )
            }
        }
    }
}

// TODO
impl error::Error for WriteError {
    fn description(&self) -> &str {
        ""
    }

    fn cause(&self) -> Option<&error::Error> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

impl<T: Write> Writer<T> {
    /// Create a new LHEF writer
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// // write to a file in LHEF version 1.0
    /// let mut file = std::fs::File::create("events.lhe").unwrap();
    /// let writer = lhef::Writer::new(
    ///    &mut file, "1.0"
    /// ).unwrap();
    /// ```
    ///
    /// ```rust
    /// // write to a byte vector
    /// let mut output = vec![];
    /// let writer = lhef::Writer::new(
    ///    std::io::Cursor::new(&mut output), "1.0"
    /// ).unwrap();
    /// ```
    pub fn new(
        mut stream: T, version: &str
    ) -> Result<Writer<T>, Box<error::Error>>
    {
        let output = String::from(LHEF_TAG_OPEN) + "\"" + version + "\">\n";
        stream.write_all(output.as_bytes())?;
        Ok(Writer{stream, state: WriterState::ExpectingHeaderOrInit})
    }

    fn assert_state(
        &self, expected: WriterState, from: &'static str
    ) -> Result<(), Box<error::Error>> {
        if self.state != expected && self.state != WriterState::Failed {
            Err(Box::new(WriteError::BadState(self.state, from)))
        }
        else {
            Ok(())
        }
    }

    fn ok_unless_failed(&self) -> Result<(), Box<error::Error>> {
        if self.state == WriterState::Failed {
            Err(Box::new(WriteError::WriteToFailed))
        }
        else {
            Ok(())
        }
    }

    /// Write a LHEF comment header
    /// # Example
    ///
    /// ```rust
    /// let mut output = vec![];
    /// let mut writer = lhef::Writer::new(
    ///    std::io::Cursor::new(&mut output), "1.0"
    /// ).unwrap();
    /// writer.header("some header text").unwrap();
    /// ```
    pub fn header(&mut self, header: &str) -> Result<(), Box<error::Error>> {
        self.assert_state(WriterState::ExpectingHeaderOrInit, "header")?;
        let output =
            String::from(COMMENT_START) + "\n" + header
            + "\n" + COMMENT_END + "\n";
        match self.stream.write_all(output.as_bytes()) {
            Ok(_) => self.ok_unless_failed(),
            Err(error) => {
                self.state = WriterState::Failed;
                Err(Box::new(error))
            }
        }
    }

    /// Write a LHEF xml header
    ///
    /// If the outermost xml tag in the argument is not "header" an
    /// additional "header" tag will be wrapped around the output. Line
    /// breaks may be added to ensure conformance with the LHEF
    /// standard.
    ///
    /// # Example
    ///
    /// ```rust
    /// let mut output = vec![];
    /// let mut writer = lhef::Writer::new(
    ///    std::io::Cursor::new(&mut output), "1.0"
    /// ).unwrap();
    /// let header = {
    ///     let mut attr = std::collections::HashMap::new();
    ///     attr.insert("attr0".to_string(), "val0".to_string());
    ///     attr.insert("attr1".to_string(), "".to_string());
    ///     lhef::XmlTree{
    ///         prefix: None,
    ///         namespace: None,
    ///         namespaces: None,
    ///         name: String::from("header"),
    ///         attributes: attr,
    ///         children: vec![],
    ///         text: Some(String::from("some xml header")),
    ///     }
    /// };
    /// writer.xml_header(&header).unwrap();
    /// ```
    pub fn xml_header(
        &mut self, header: &XmlTree
    ) -> Result<(), Box<error::Error>> {
        self.assert_state(WriterState::ExpectingHeaderOrInit, "xml header")?;
        let mut output = String::from(HEADER_START);
        if header.name != "header" {
            output += ">\n";
            xml_to_string(&header, &mut output);
            output += "\n";
        }
        else {
            for (key, value) in &header.attributes {
                write!(&mut output, " {}=\"{}\"", key, value)?;
            }
            output += ">";
            if !header.children.is_empty() {
                output += "\n";
                for child in &header.children {
                    xml_to_string(&child, &mut output)
                }
            }
            match header.text {
                None => output += "\n",
                Some(ref text) => {
                    if header.children.is_empty() && !text.starts_with("\n") {
                        output += "\n"
                    }
                    output += text;
                    if !text.ends_with("\n") {
                        output += "\n";
                    }
                }
            };
        }
        output += HEADER_END;
        output += "\n";
        match self.stream.write_all(output.as_bytes()) {
            Ok(_) => self.ok_unless_failed(),
            Err(error) => {
                self.state = WriterState::Failed;
                Err(Box::new(error))
            }
        }
    }

    /// Write the run information in HEPRUP format
    ///
    /// # Example
    ///
    /// ```rust
    /// let mut output = vec![];
    /// let mut writer = lhef::Writer::new(
    ///    std::io::Cursor::new(&mut output), "1.0"
    /// ).unwrap();
    /// let heprup = lhef::HEPRUP {
    ///     IDBMUP: [2212, 2212],
    ///     EBMUP: [7000.0, 7000.0],
    ///     PDFGUP: [0, 0],
    ///     PDFSUP: [230000, 230000],
    ///     IDWTUP: 2,
    ///     NPRUP: 1,
    ///     XSECUP: vec!(120588124.02),
    ///     XERRUP: vec!(702517.48228),
    ///     XMAXUP: vec!(94290.49),
    ///     LPRUP:  vec!(1),
    ///     info: String::new(),
    ///     attr: lhef::XmlAttr::new(),
    /// };
    /// writer.heprup(&heprup).unwrap();
    /// ```
    pub fn heprup(
        &mut self, runinfo: &HEPRUP
    ) -> Result<(), Box<error::Error>> {
        self.assert_state(WriterState::ExpectingHeaderOrInit, "init")?;
        let num_sub = runinfo.NPRUP as usize;
        if
            num_sub != runinfo.XSECUP.len()
            || num_sub != runinfo.XERRUP.len()
            || num_sub != runinfo.XMAXUP.len()
            || num_sub != runinfo.LPRUP.len()
        {
            return Err(Box::new(WriteError::MismatchedSubprocesses))
        }
        let mut output = String::from(INIT_START);
        for (attr, value) in &runinfo.attr {
            write!(&mut output, "{}=\"{}\"", attr, value)?;
        }
        output += ">\n";
        for entry in runinfo.IDBMUP.iter() {
            write!(&mut output, "{} ", entry)?;
        }
        for entry in runinfo.EBMUP.iter() {
            write!(&mut output, "{} ", entry)?;
        }
        for entry in runinfo.PDFGUP.iter() {
            write!(&mut output, "{} ", entry)?;
        }
        for entry in runinfo.PDFSUP.iter() {
            write!(&mut output, "{} ", entry)?;
        }
        write!(&mut output, "{} ", runinfo.IDWTUP)?;
        write!(&mut output, "{}\n", runinfo.NPRUP)?;
        let subprocess_infos = izip!(
            &runinfo.XSECUP, &runinfo.XERRUP, &runinfo.XMAXUP, &runinfo.LPRUP
        );
        for (xs, xserr, xsmax, id) in subprocess_infos {
            write!(&mut output, "{} {} {} {}\n", xs, xserr, xsmax, id)?;
        }
        if !runinfo.info.is_empty() {
            output += &runinfo.info;
            if runinfo.info.chars().last() != Some('\n') {
                output += "\n"
            }
        }
        output += INIT_END;
        output += "\n";
        if let Err(error) = self.stream.write_all(output.as_bytes()) {
            self.state = WriterState::Failed;
            return Err(Box::new(error))
        }
        if self.state != WriterState::Failed {
            self.state = WriterState::ExpectingEventOrFinish
        }
        self.ok_unless_failed()
    }

    /// Write event in HEPEUP format
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let mut output = vec![];
    /// let mut writer = lhef::Writer::new(
    ///    std::io::Cursor::new(&mut output), "1.0"
    /// ).unwrap();
    /// // ... write run information here ...
    /// let hepeup = lhef::HEPEUP {
    ///     NUP: 4,
    ///     IDRUP: 1,
    ///     XWGTUP: 84515.12,
    ///     SCALUP: 91.188,
    ///     AQEDUP: 0.007546771,
    ///     AQCDUP: 0.1190024,
    ///     IDUP: vec!(1, 21, 21, 1),
    ///     ISTUP: vec!(-1, -1, 1, 1),
    ///     MOTHUP: vec!([0, 0], [0, 0], [1, 2], [1, 2]),
    ///     ICOLUP: vec!([503, 0], [501, 502], [503, 502], [501, 0]),
    ///     PUP: vec!(
    ///         [0.0, 0.0, 4.7789443449, 4.7789443449, 0.0],
    ///         [0.0, 0.0, -1240.3761329, 1240.3761329, 0.0],
    ///         [37.283715118, 21.98166528, -1132.689358, 1133.5159684, 0.0],
    ///         [-37.283715118, -21.98166528, -102.90783056, 111.63910879, 0.0]
    ///     ),
    ///     VTIMUP: vec!(0.0, 0.0, 0.0, 0.0),
    ///     SPINUP: vec!(1.0, -1.0, -1.0, 1.0),
    ///     info: String::new(),
    ///     attr: lhef::XmlAttr::new(),
    /// };
    /// writer.hepeup(&hepeup).unwrap();
    /// ```
    pub fn hepeup(
        &mut self, event: &HEPEUP
    ) -> Result<(), Box<error::Error>> {
        self.assert_state(WriterState::ExpectingEventOrFinish, "event")?;
        let num_particles = event.NUP as usize;
        if
               num_particles != event.IDUP.len()
            || num_particles != event.ISTUP.len()
            || num_particles != event.MOTHUP.len()
            || num_particles != event.ICOLUP.len()
            || num_particles != event.PUP.len()
            || num_particles != event.VTIMUP.len()
            || num_particles != event.SPINUP.len()
        {
            return Err(Box::new(WriteError::MismatchedParticles))
        }
        let mut output = String::from(EVENT_START);
        for (attr, value) in &event.attr {
            write!(&mut output, " {}=\"{}\"", attr, value)?;
        }
        output += ">\n";
        write!(
            &mut output, "{} {} {} {} {} {} ",
            event.NUP, event.IDRUP, event.XWGTUP,
            event.SCALUP, event.AQEDUP, event.AQCDUP
        )?;
        output += ">\n";
        let particles = izip!(
            &event.IDUP, &event.ISTUP, &event.MOTHUP, &event.ICOLUP,
            &event.PUP, &event.VTIMUP, &event.SPINUP,
        );
        for (id, status, mothers, colour, p, lifetime, spin) in particles {
            write!(&mut output, "{} {} ", id, status)?;
            for m in mothers {
                write!(&mut output, "{} ", m)?;
            }
            for c in colour {
                write!(&mut output, "{} ", c)?;
            }
            for p in p {
                write!(&mut output, "{} ", p)?;
            }
            write!(&mut output, "{} {}\n", lifetime, spin)?;
        }
        if !event.info.is_empty() {
            output += &event.info;
            if event.info.chars().last() != Some('\n') {
                output += "\n"
            }
        }
        output += EVENT_END;
        output += "\n";
        match self.stream.write_all(output.as_bytes()) {
            Ok(_) => self.ok_unless_failed(),
            Err(error) => {
                self.state = WriterState::Failed;
                Err(Box::new(error))
            }
        }
    }

    /// Close LHEF output
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// let mut output = vec![];
    /// let mut writer = lhef::Writer::new(
    ///    std::io::Cursor::new(&mut output), "1.0"
    /// ).unwrap();
    /// // ... write header, run information, events ...
    /// writer.finish().unwrap();
    /// ```
    pub fn finish(&mut self) -> Result<(), Box<error::Error>> {
        self.assert_state(WriterState::ExpectingEventOrFinish, "finish")?;
        let output = String::from(LHEF_LAST_LINE) + "\n";
        if let Err(error) = self.stream.write_all(output.as_bytes()) {
            self.state = WriterState::Failed;
            return Err(Box::new(error))
        }
        if self.state != WriterState::Failed {
            self.state = WriterState::Finished
        }
        self.ok_unless_failed()
    }
}

impl<T: Write> Drop for Writer<T> {
    fn drop(&mut self) {
        if self.state == WriterState::ExpectingEventOrFinish {
            let _ = self.finish();
        }
    }
}

#[cfg(test)]
mod reader_tests {
    extern crate flate2;
    use super::*;

    use std::fs::File;
    use std::io::BufReader;
    use reader_tests::flate2::bufread::GzDecoder;

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
        while let Ok(Some(_)) = lhef.hepeup() { nevents += 1 };
        assert_eq!(nevents, 1628);
    }

    #[test]
    fn read_hejfog() {
        let file = File::open("test_data/HEJFOG.lhe.gz").expect("file not found");
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
        while let Ok(Some(_)) = lhef.hepeup() { nevents += 1 };
        assert_eq!(nevents, 10);
    }
}

#[cfg(test)]
mod writer_tests {
    use super::*;
    use std::io;
    use std::collections::HashMap;
    use std::str;

    #[test]
    fn write() {
        let heprup = HEPRUP {
            IDBMUP: [2212, 2212],
            EBMUP: [7000.0, 7000.0],
            PDFGUP: [0, 0],
            PDFSUP: [230000, 230000],
            IDWTUP: 2,
            NPRUP: 1,
            XSECUP: vec!(120588124.02),
            XERRUP: vec!(702517.48228),
            XMAXUP: vec!(94290.49),
            LPRUP:  vec!(1),
            info: String::new(),
            attr: XmlAttr::new(),
        };
        let hepeup = HEPEUP {
            NUP: 4,
            IDRUP: 1,
            XWGTUP: 84515.12,
            SCALUP: 91.188,
            AQEDUP: 0.007546771,
            AQCDUP: 0.1190024,
            IDUP: vec!(1, 21, 21, 1),
            ISTUP: vec!(-1, -1, 1, 1),
            MOTHUP: vec!([0, 0], [0, 0], [1, 2], [1, 2]),
            ICOLUP: vec!([503, 0], [501, 502], [503, 502], [501, 0]),
            PUP: vec!(
                [0.0, 0.0, 4.7789443449, 4.7789443449, 0.0],
                [0.0, 0.0, -1240.3761329, 1240.3761329, 0.0],
                [37.283715118, 21.98166528, -1132.689358, 1133.5159684, 0.0],
                [-37.283715118, -21.98166528, -102.90783056, 111.63910879, 0.0]
            ),
            VTIMUP: vec!(0.0, 0.0, 0.0, 0.0),
            SPINUP: vec!(1.0, -1.0, -1.0, 1.0),
            info: String::from(
                "<mgrwt>
<rscale>  2 0.91188000E+02</rscale>
<asrwt>0</asrwt>
<pdfrwt beam=\"1\">  1       21 0.17719659E+00 0.91188000E+02</pdfrwt>
<pdfrwt beam=\"2\">  1        1 0.68270633E-03 0.91188000E+02</pdfrwt>
<totfact> 0.49322010E+04</totfact>
</mgrwt>
"
            ),
            attr: XmlAttr::new(),
        };
        let mut buf = vec![];
        {
            let mut writer = Writer::new(
                io::Cursor::new(&mut buf), "1.0"
            ).unwrap();
            writer.header("some header").unwrap();
            let header = {
                let mut attr = HashMap::new();
                attr.insert("attr0".to_string(), "val0".to_string());
                attr.insert("attr1".to_string(), "".to_string());
                XmlTree{
                    prefix: None,
                    namespace: None,
                    namespaces: None,
                    name: String::from("header"),
                    attributes: attr,
                    children: vec![],
                    text: Some(String::from("some xml header")),
                }
            };
            writer.xml_header(&header).unwrap();
            writer.heprup(&heprup).unwrap();
            writer.hepeup(&hepeup).unwrap();
            writer.finish().unwrap();
        }
        // println!("{}", str::from_utf8(&buf).unwrap());
    }
}

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
