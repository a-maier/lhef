use std::error;
use std::fmt;
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::ops::Drop;
use std::str;

use crate::syntax::*;
use crate::data::*;

use itertools::izip;

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
#[derive(Debug, PartialEq, Eq)]
pub struct Writer<T: Write> {
    stream: T,
    state: WriterState,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Hash, PartialEq, Eq, Copy)]
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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
            MismatchedSubprocesses => write!(
                f,
                "Mismatch between NPRUP and length of at least one of \
                 XSECUP, XERRUP, XMAXUP, LPRUP."
            ),
            MismatchedParticles => write!(
                f,
                "Mismatch between NUP and length of at least one of \
                 IDUP, ISTUP, MOTHUP, ICOLUP, PUP, VTIMUP, SPINUP."
            ),
            BadState(ref state, attempt) => write!(
                f,
                "Writer is in state '{:?}', cannot write '{}'.",
                state, attempt
            ),
            WriteToFailed => write!(
                f,
                "Writer is in 'Failed' state. \
                 Output was written, but the file may be broken anyway."
            ),
        }
    }
}

impl error::Error for WriteError {}

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
        mut stream: T,
        version: &str,
    ) -> Result<Writer<T>, Box<dyn error::Error>> {
        let output = String::from(LHEF_TAG_OPEN) + "\"" + version + "\">\n";
        stream.write_all(output.as_bytes())?;
        Ok(Writer {
            stream,
            state: WriterState::ExpectingHeaderOrInit,
        })
    }

    fn assert_state(
        &self,
        expected: WriterState,
        from: &'static str,
    ) -> Result<(), Box<dyn error::Error>> {
        if self.state != expected && self.state != WriterState::Failed {
            Err(Box::new(WriteError::BadState(self.state, from)))
        } else {
            Ok(())
        }
    }

    fn ok_unless_failed(&self) -> Result<(), Box<dyn error::Error>> {
        if self.state == WriterState::Failed {
            Err(Box::new(WriteError::WriteToFailed))
        } else {
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
    pub fn header(
        &mut self,
        header: &str
    ) -> Result<(), Box<dyn error::Error>> {
        self.assert_state(WriterState::ExpectingHeaderOrInit, "header")?;
        let output = String::from(COMMENT_START)
            + "\n"
            + header
            + "\n"
            + COMMENT_END
            + "\n";
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
        &mut self,
        header: &XmlTree,
    ) -> Result<(), Box<dyn error::Error>> {
        self.assert_state(WriterState::ExpectingHeaderOrInit, "xml header")?;
        let mut output = String::from(HEADER_START);
        if header.name != "header" {
            output += ">\n";
            xml_to_string(header, &mut output);
            output += "\n";
        } else {
            for (key, value) in &header.attributes {
                write!(&mut output, " {}=\"{}\"", key, value)?;
            }
            output += ">";
            if !header.children.is_empty() {
                output += "\n";
                for child in &header.children {
                    xml_to_string(child, &mut output)
                }
            }
            match header.text {
                None => output += "\n",
                Some(ref text) => {
                    if header.children.is_empty() && !text.starts_with('\n') {
                        output += "\n"
                    }
                    output += text;
                    if !text.ends_with('\n') {
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
        &mut self,
        runinfo: &HEPRUP,
    ) -> Result<(), Box<dyn error::Error>> {
        self.assert_state(WriterState::ExpectingHeaderOrInit, "init")?;
        let num_sub = runinfo.NPRUP as usize;
        if num_sub != runinfo.XSECUP.len()
            || num_sub != runinfo.XERRUP.len()
            || num_sub != runinfo.XMAXUP.len()
            || num_sub != runinfo.LPRUP.len()
        {
            return Err(Box::new(WriteError::MismatchedSubprocesses));
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
        writeln!(&mut output, "{}", runinfo.NPRUP)?;
        let subprocess_infos = izip!(
            &runinfo.XSECUP,
            &runinfo.XERRUP,
            &runinfo.XMAXUP,
            &runinfo.LPRUP
        );
        for (xs, xserr, xsmax, id) in subprocess_infos {
            writeln!(&mut output, "{} {} {} {}", xs, xserr, xsmax, id)?;
        }
        if !runinfo.info.is_empty() {
            output += &runinfo.info;
            if !runinfo.info.ends_with('\n') {
                output += "\n"
            }
        }
        output += INIT_END;
        output += "\n";
        if let Err(error) = self.stream.write_all(output.as_bytes()) {
            self.state = WriterState::Failed;
            return Err(Box::new(error));
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
        &mut self,
        event: &HEPEUP
    ) -> Result<(), Box<dyn error::Error>> {
        let mut buffer = ryu::Buffer::new();
        self.assert_state(WriterState::ExpectingEventOrFinish, "event")?;
        let num_particles = event.NUP as usize;
        if num_particles != event.IDUP.len()
            || num_particles != event.ISTUP.len()
            || num_particles != event.MOTHUP.len()
            || num_particles != event.ICOLUP.len()
            || num_particles != event.PUP.len()
            || num_particles != event.VTIMUP.len()
            || num_particles != event.SPINUP.len()
        {
            return Err(Box::new(WriteError::MismatchedParticles));
        }
        let mut output = String::from(EVENT_START);
        for (attr, value) in &event.attr {
            write!(&mut output, " {}=\"{}\"", attr, value)?;
        }
        output += ">\n";
        writeln!(
            &mut output,
            "{} {} {} {} {} {}",
            event.NUP,
            event.IDRUP,
            buffer.format(event.XWGTUP),
            ryu::Buffer::new().format(event.SCALUP),
            ryu::Buffer::new().format(event.AQEDUP),
            ryu::Buffer::new().format(event.AQCDUP)
        )?;
        let particles = izip!(
            &event.IDUP,
            &event.ISTUP,
            &event.MOTHUP,
            &event.ICOLUP,
            &event.PUP,
            &event.VTIMUP,
            &event.SPINUP,
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
                write!(&mut output, "{} ", buffer.format(*p))?;
            }
            write!(&mut output, "{} ", buffer.format(*lifetime))?;
            writeln!(&mut output, "{}", buffer.format(*spin))?;
        }
        if !event.info.is_empty() {
            output += &event.info;
            if !event.info.ends_with('\n') {
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
    pub fn finish(&mut self) -> Result<(), Box<dyn error::Error>> {
        self.assert_state(WriterState::ExpectingEventOrFinish, "finish")?;
        let output = String::from(LHEF_LAST_LINE) + "\n";
        if let Err(error) = self.stream.write_all(output.as_bytes()) {
            self.state = WriterState::Failed;
            return Err(Box::new(error));
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
mod writer_tests {
    use super::*;
    use std::collections::HashMap;
    use std::io;

    #[test]
    fn write() {
        let heprup = HEPRUP {
            IDBMUP: [2212, 2212],
            EBMUP: [7000.0, 7000.0],
            PDFGUP: [0, 0],
            PDFSUP: [230000, 230000],
            IDWTUP: 2,
            NPRUP: 1,
            XSECUP: vec![120588124.02],
            XERRUP: vec![702517.48228],
            XMAXUP: vec![94290.49],
            LPRUP: vec![1],
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
            IDUP: vec![1, 21, 21, 1],
            ISTUP: vec![-1, -1, 1, 1],
            MOTHUP: vec![[0, 0], [0, 0], [1, 2], [1, 2]],
            ICOLUP: vec![[503, 0], [501, 502], [503, 502], [501, 0]],
            PUP: vec![
                [0.0, 0.0, 4.7789443449, 4.7789443449, 0.0],
                [0.0, 0.0, -1240.3761329, 1240.3761329, 0.0],
                [37.283715118, 21.98166528, -1132.689358, 1133.5159684, 0.0],
                [
                    -37.283715118,
                    -21.98166528,
                    -102.90783056,
                    111.63910879,
                    0.0,
                ],
            ],
            VTIMUP: vec![0.0, 0.0, 0.0, 0.0],
            SPINUP: vec![1.0, -1.0, -1.0, 1.0],
            info: String::from(
                "<mgrwt>
<rscale>  2 0.91188000E+02</rscale>
<asrwt>0</asrwt>
<pdfrwt beam=\"1\">  1       21 0.17719659E+00 0.91188000E+02</pdfrwt>
<pdfrwt beam=\"2\">  1        1 0.68270633E-03 0.91188000E+02</pdfrwt>
<totfact> 0.49322010E+04</totfact>
</mgrwt>
",
            ),
            attr: XmlAttr::new(),
        };
        let mut buf = vec![];
        {
            let mut writer =
                Writer::new(io::Cursor::new(&mut buf), "1.0").unwrap();
            writer.header("some header").unwrap();
            let header = {
                let mut attr = HashMap::new();
                attr.insert("attr0".to_string(), "val0".to_string());
                attr.insert("attr1".to_string(), "".to_string());
                XmlTree {
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
        xml_to_string(child, output)
    }
    *output += &format!("</{}>", xml.name);
}
