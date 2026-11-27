//! Resistor math with no GUI dependencies.
//!
//! - [`color`]: IEC 60062 color table
//! - [`bands`]: decode and encode 1/3/4/5/6-band color codes
//! - [`smd`]: 3-digit, 4-digit, R-notation and EIA-96 SMD markings
//! - [`eseries`]: IEC 60063 preferred values (E3 to E192)
//! - [`combo`]: best two-resistor series or parallel pair for a target
//! - [`value`]: parse and format resistance values
//! - [`ohm`]: Ohm's law and power
//! - [`power`]: package ratings, derating and voltage limits
//! - [`divider`]: voltage dividers and the best standard-value pair for a ratio
//! - [`led`]: LED series resistor
//! - [`network`]: any number of resistors in series or parallel, and three-resistor combinations
//! - [`tolerance`]: worst-case and RSS tolerance analysis
//! - [`thermal`]: temperature coefficient drift and Johnson noise
//! - [`rtd`]: Pt100/Pt1000 to IEC 60751
//! - [`ntc`]: NTC thermistors (Beta and Steinhart–Hart)
//! - [`current_loop`]: 4–20 mA scaling, NAMUR NE43, loop budget and HART
//! - [`bridge`]: Wheatstone bridge
//! - [`wire`]: wire and PCB trace resistance, IPC-2221 trace current

pub mod bands;
pub mod bridge;
pub mod color;
pub mod combo;
pub mod current_loop;
pub mod divider;
pub mod error;
pub mod eseries;
pub mod led;
pub mod network;
pub mod ntc;
pub mod ohm;
pub mod power;
pub mod rtd;
pub mod smd;
pub mod thermal;
pub mod tolerance;
pub mod value;
pub mod wire;

pub use bands::{BandCount, Resistor};
pub use color::Color;
pub use error::Error;
pub use eseries::ESeries;
