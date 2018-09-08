#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;
use std::io::BufRead;
use std::fmt;
use std::error;

const LHEF_TAG_OPEN: &'static str = "<LesHouchesEvents version=";
const COMMENT_START: &'static str = "<!--";
const COMMENT_END: &'static str = "-->";
const HEADER_START: &'static str = "<header>";
const HEADER_END: &'static str = "</header>";
const INIT_START: &'static str = "<init>";
const INIT_END: &'static str = "</init>";
const EVENT_START: &'static str = "<event>";
const EVENT_END: &'static str = "</event>";
const LHEF_LAST_LINE: &'static str = "</LesHouchesEvents>";

pub struct LesHouchesReader<Stream> {
    stream: Stream,
    header: String, // TODO: or some xml struct?
    heprup: HEPRUP,
}

impl<Stream: BufRead> LesHouchesReader<Stream> {
    pub fn new(mut stream: Stream) -> Result<LesHouchesReader<Stream>, Box<error::Error>> {
        check_first_line(&mut stream)?;
        let header = parse_header(&mut stream)?;
        let heprup = parse_init(&mut stream)?;
        Ok(LesHouchesReader{stream, header, heprup})
    }

    pub fn header(&self) -> &str {
        &self.header
    }

    pub fn heprup(&self) -> &HEPRUP {
        &self.heprup
    }

    pub fn event(&mut self) -> Result<Option<HEPEUP>, Box<error::Error>> {
        let mut line = String::new();
        self.stream.read_line(&mut line)?;
        match line.trim() {
            EVENT_START => Ok(Some(parse_event(&mut self.stream)?)),
            LHEF_LAST_LINE => Ok(None),
            _ => Err(Box::new(ParseError::BadEventStart(line)))
        }
    }
}

fn check_first_line<Stream: BufRead>(stream: &mut Stream) -> Result<(), Box<error::Error>> {
    use ParseError::*;
    let mut first_line = String::new();
    stream.read_line(&mut first_line)?;
    let line_cp = first_line.clone();
    let mut line_entries = first_line.trim().split('"');
    if line_entries.next() != Some(LHEF_TAG_OPEN) {
        return Err(Box::new(ParseError::BadFirstLine(line_cp)))
    }
    match line_entries.next() {
        Some("1.0") => {},
        Some("2.0") => {},
        Some("3.0") => {},
        Some(version) => {
            return Err(Box::new(UnsupportedVersion(version.to_string())))
        }
        None => {
            return Err(Box::new(MissingVersion))
        }
    }
    if line_entries.next() != Some(">") {
        return Err(Box::new(ParseError::BadFirstLine(line_cp)))
    }
    Ok(())
}

fn parse_header<Stream: BufRead>(mut stream: &mut Stream) -> Result<String, Box<error::Error>> {
    let mut header = String::new();
    loop {
        stream.read_line(&mut header)?;
        match header.lines().last().unwrap().trim() {
            COMMENT_START => parse_comment_header(&mut stream, &mut header)?,
            HEADER_START => parse_structured_header(&mut stream, &mut header)?,
            INIT_START => {
                pop_line(&mut header);
                return Ok(header)
            },
            _ => return Err(Box::new(ParseError::BadHeaderStart(
                header.lines().last().unwrap().trim().to_owned()
            ))),
        };
    }
}

fn pop_line(s: &mut String) {
    s.pop();
    while !s.is_empty() && s.chars().last().unwrap() != '\n' {
        s.pop();
    }
}

fn parse_comment_header<Stream: BufRead>(
    stream: &mut Stream, header: &mut String
) -> Result<(), Box<error::Error>> {
    loop {
        if stream.read_line(header)? == 0 {
            return Err(Box::new(ParseError::EndOfFile("header")));
        }
        if header.lines().last().unwrap().trim() == COMMENT_END {
            return Ok(())
        }
    }
}

//TODO: parse as xml
fn parse_structured_header<Stream: BufRead>(
    stream: &mut Stream, header: &mut String
) -> Result<(), Box<error::Error>> {
    loop {
        if stream.read_line(header)? == 0 {
            return Err(Box::new(ParseError::EndOfFile("header")));
        }
        if header.lines().last().unwrap().trim() == HEADER_END {
            return Ok(())
        };
    }
}

fn parse<T>(name: &str, text: Option<&str>) -> Result<T, Box<error::Error>>
where T: std::str::FromStr {
    use ParseError::*;
    let text: &str = text.ok_or(Box::new(MissingEntry(String::from(name))))?;
    match text.parse::<T>() {
        Ok(t) => Ok(t),
        Err(_) => Err(Box::new(ConversionError(text.to_owned())))
    }
}

#[allow(non_snake_case)]
fn parse_init<Stream: BufRead>(
    stream: &mut Stream
) -> Result<HEPRUP, Box<error::Error>> {
    // we have already consumed to opening <init> when reading the header
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
    Ok(HEPRUP{
        IDBMUP, EBMUP, PDFGUP, PDFSUP, IDWTUP, NPRUP,
        XSECUP, XERRUP, XMAXUP, LPRUP,
        info
    })
}

#[allow(non_snake_case)]
fn parse_event<Stream: BufRead>(
    stream: &mut Stream
) -> Result<HEPEUP, Box<error::Error>> {
    // we have already consumed to opening <event>
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
    Ok(HEPEUP{
        NUP, IDRUP, XWGTUP, SCALUP, AQEDUP, AQCDUP,
        IDUP, ISTUP, MOTHUP, ICOLUP, PUP, VTIMUP, SPINUP,
        info
    })
}

#[allow(non_snake_case)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(PartialEq,Debug,Clone)]
pub struct HEPRUP {
    IDBMUP: [i32; 2],
    EBMUP: [f64; 2],
    PDFGUP: [i32; 2],
    PDFSUP: [i32; 2],
    IDWTUP: i32,
    NPRUP: i32,
    XSECUP: Vec<f64>,
    XERRUP: Vec<f64>,
    XMAXUP: Vec<f64>,
    LPRUP: Vec<i32>,
    info: String,
}

#[allow(non_snake_case)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(PartialEq,Debug,Clone)]
pub struct HEPEUP{
    NUP: i32,
    IDRUP: i32,
    XWGTUP: f64,
    SCALUP: f64,
    AQEDUP: f64,
    AQCDUP: f64,
    IDUP: Vec<i32>,
    ISTUP: Vec<i32>,
    MOTHUP: Vec<[i32; 2]>,
    ICOLUP: Vec<[i32; 2]>,
    PUP: Vec<[f64; 5]>,
    VTIMUP: Vec<f64>,
    SPINUP: Vec<f64>,
    info: String,
}

#[derive(Debug)]
enum ParseError {
    BadFirstLine(String),
    BadHeaderStart(String),
    BadEventStart(String),
    MissingEntry(String),
    ConversionError(String),
    UnsupportedVersion(String),
    MissingVersion,
    EndOfFile(&'static str),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ParseError::*;
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

#[cfg(test)]
mod tests {
    extern crate flate2;
    use super::*;

    use std::fs::File;
    use std::io::BufReader;
    use tests::flate2::bufread::GzDecoder;

    #[test]
    fn read_correct() {
        let file = File::open("test_data/2j.lhe.gz").expect("file not found");
        let reader = BufReader::new(GzDecoder::new(BufReader::new(file)));
        let mut lhef = LesHouchesReader::new(reader).unwrap();
        let mut nevents = 0;
        while let Ok(Some(_)) = lhef.event() { nevents += 1 };
        assert_eq!(nevents, 1628);
    }

    #[test]
    fn read_hejfog() {
        let file = File::open("test_data/HEJFOG.lhe.gz").expect("file not found");
        let reader = BufReader::new(GzDecoder::new(BufReader::new(file)));
        let mut lhef = LesHouchesReader::new(reader).unwrap();
        let mut nevents = 0;
        while let Ok(Some(_)) = lhef.event() { nevents += 1 };
        assert_eq!(nevents, 10);
    }
}
