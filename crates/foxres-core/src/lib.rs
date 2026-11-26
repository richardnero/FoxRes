//! Resistor math with no GUI dependencies.
//!
//! - [`color`]: IEC 60062 color table
//! - [`bands`]: decode and encode 1/3/4/5/6-band color codes
//! - [`smd`]: 3-digit, 4-digit, R-notation and EIA-96 SMD markings
//! - [`eseries`]: IEC 60063 preferred values (E3 to E192)
//! - [`combo`]: best two-resistor series or parallel pair for a target
//! - [`value`]: parse and format resistance values

pub mod bands;
pub mod color;
pub mod combo;
pub mod error;
pub mod eseries;
pub mod smd;
pub mod value;

pub use bands::{BandCount, Resistor};
pub use color::Color;
pub use error::Error;
pub use eseries::ESeries;
