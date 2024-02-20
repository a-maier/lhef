use std::collections::HashMap;

pub type XmlAttr = HashMap<String, String>;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Generator run information
///
/// See <https://arxiv.org/abs/hep-ph/0109068v1> for details on the fields.
#[allow(non_snake_case)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(PartialEq, Debug, Clone)]
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
    /// Attributes in `<init>` tag
    pub attr: XmlAttr,
}

/// Event information
///
/// See <https://arxiv.org/abs/hep-ph/0109068v1> for details on the fields.
#[allow(non_snake_case)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(PartialEq, Debug, Clone)]
pub struct HEPEUP {
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
    /// Attributes in `<event>` tag
    pub attr: XmlAttr,
}

pub type XmlTree = xmltree::Element;
