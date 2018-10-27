#[macro_use]
extern crate itertools;

pub mod fortran_blocks;
pub mod reader;
pub mod writer;
mod tags;

pub use fortran_blocks::*;
pub use reader::*;
pub use writer::*;
