use std::io::Write;
use tags::*;
use fortran_blocks::*;
use std::error;
use std::fmt;
use std::io;
use xmltree;

pub type XmlTree = xmltree::Element;

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
#[derive(Debug)]
pub struct Writer<T: Write> {
    stream: T,
}

#[derive(Debug)]
enum WriteError {
    MismatchedSubprocesses,
    MismatchedParticles,
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::WriteError::*;
        match *self {
            MismatchedSubprocesses => {
                write!(
                    f,
                    "Mismatch between NPRUP and length of at least one of \
                     XSECUP, XERRUP, XMAXUP, LPRUP"
                )
            },
            MismatchedParticles => {
                write!(
                    f,
                    "Mismatch between NUP and length of at least one of \
                     IDUP, ISTUP, MOTHUP, ICOLUP, PUP, VTIMUP, SPINUP"
                )
            },
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
        let output = [LHEF_TAG_OPEN, "\"", version, "\">\n"];
        for text in &output {
            stream.write_all(text.as_bytes())?;
        }
        Ok(Writer{stream})
    }

    fn write<U: fmt::Display + ?Sized> (
        &mut self, expr: &U
    )  -> Result<(), io::Error> {
        self.stream.write_all(format!("{}", expr).as_bytes())
    }

    fn write_field<U: fmt::Display + ?Sized> (
        &mut self, expr: &U
    )  -> Result<(), io::Error> {
        self.stream.write_all(format!("{} ", expr).as_bytes())
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
        let output = [COMMENT_START, "\n", header, "\n", COMMENT_END, "\n"];
        for text in &output {
            self.write(text)?;
        }
        Ok(())
    }

    fn write_xml(
        &mut self, xml: &XmlTree
    ) -> Result<(), Box<error::Error>> {
        self.write(&format!("<{}", xml.name))?;
        for (key, value) in &xml.attributes {
            self.write(&format!(" {}=\"{}\"", key, value))?;
        }
        self.write(">")?;
        if let Some(ref text) = xml.text {
            self.write(text)?;
        }
        for child in &xml.children {
            self.write_xml(child)?
        }
        self.write(&format!("</{}>", xml.name))?;
        Ok(())
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
        self.write(HEADER_START)?;
        if header.name != "header" {
            self.write(">\n")?;
            self.write_xml(header)?;
            self.write("\n")?;
        }
        else {
            for (key, value) in &header.attributes {
                self.write(&format!(" {}=\"{}\"", key, value))?;
            }
            self.write(">")?;
            if !header.children.is_empty() {
                self.write("\n")?;
                for child in &header.children {
                    self.write_xml(child)?;
                }
            }
            match header.text {
                None => self.write("\n")?,
                Some(ref text) => {
                    if header.children.is_empty() && !text.starts_with("\n") {
                        self.write("\n")?;
                    }
                    self.write(text)?;
                    if !text.ends_with("\n") {
                        self.write("\n")?;
                    }
                }
            };
        }
        self.write(HEADER_END)?;
        self.write("\n")?;
        Ok(())
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
        let num_sub = runinfo.NPRUP as usize;
        if
            num_sub != runinfo.XSECUP.len()
            || num_sub != runinfo.XERRUP.len()
            || num_sub != runinfo.XMAXUP.len()
            || num_sub != runinfo.LPRUP.len()
        {
            return Err(Box::new(WriteError::MismatchedSubprocesses))
        }
        self.write(INIT_START)?;
        for (attr, value) in &runinfo.attr {
            self.write(&format!("{} = \"{}\"", attr, value))?;
        }
        self.write(">\n")?;
        for entry in runinfo.IDBMUP.iter() {
            self.write_field(entry)?;
        }
        for entry in runinfo.EBMUP.iter() {
            self.write_field(entry)?;
        }
        for entry in runinfo.PDFGUP.iter() {
            self.write_field(entry)?;
        }
        for entry in runinfo.PDFSUP.iter() {
            self.write_field(entry)?;
        }
        self.write_field(&runinfo.IDWTUP)?;
        self.write(&runinfo.NPRUP)?;
        self.write(&'\n')?;
        let subprocess_infos = izip!(
            &runinfo.XSECUP, &runinfo.XERRUP, &runinfo.XMAXUP, &runinfo.LPRUP
        );
        for (xs, xserr, xsmax, id) in subprocess_infos {
            self.write_field(xs)?;
            self.write_field(xserr)?;
            self.write_field(xsmax)?;
            self.write(id)?;
            self.write(&'\n')?;
        }
        if !runinfo.info.is_empty() {
            self.write(&runinfo.info)?;
            if runinfo.info.chars().last() != Some('\n') {
                self.write(&'\n')?;
            }
        }
        self.write(INIT_END)?;
        self.write(&'\n')?;
        Ok(())
    }

    /// Write event in HEPEUP format
    ///
    /// # Example
    ///
    /// ```rust
    /// let mut output = vec![];
    /// let mut writer = lhef::Writer::new(
    ///    std::io::Cursor::new(&mut output), "1.0"
    /// ).unwrap();
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
        self.write(EVENT_START)?;
        for (attr, value) in &event.attr {
            self.write(&format!("{} = \"{}\"", attr, value))?;
        }
        self.write(">\n")?;
        self.write_field(&event.NUP)?;
        self.write_field(&event.IDRUP)?;
        self.write_field(&event.XWGTUP)?;
        self.write_field(&event.SCALUP)?;
        self.write_field(&event.AQEDUP)?;
        self.write_field(&event.AQCDUP)?;
        self.write("\n")?;
        let particles = izip!(
            &event.IDUP, &event.ISTUP, &event.MOTHUP, &event.ICOLUP,
            &event.PUP, &event.VTIMUP, &event.SPINUP,
        );
        for (id, status, mothers, colour, p, lifetime, spin) in particles {
            self.write_field(id)?;
            self.write_field(status)?;
            for m in mothers { self.write_field(m)? }
            for c in colour { self.write_field(c)? }
            for p in p { self.write_field(p)? }
            self.write_field(lifetime)?;
            self.write(spin)?;
            self.write(&'\n')?;
        }
        if !event.info.is_empty() {
            self.write(&event.info)?;
            if event.info.chars().last() != Some('\n') {
                self.write(&'\n')?;
            }
        }
        self.write(EVENT_END)?;
        self.write(&'\n')?;
        Ok(())
    }

    /// Close LHEF output
    ///
    /// # Example
    ///
    /// ```rust
    /// let mut output = vec![];
    /// let mut writer = lhef::Writer::new(
    ///    std::io::Cursor::new(&mut output), "1.0"
    /// ).unwrap();
    /// writer.finish().unwrap();
    /// ```
    pub fn finish(&mut self) -> Result<(), Box<error::Error>> {
        self.write(LHEF_LAST_LINE)?;
        self.write("\n")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
