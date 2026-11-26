use std::fmt;

/// Errors returned by `foxres-core`.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The number of bands given is not 1, 3, 4, 5 or 6.
    BandCount(usize),
    /// A color was used in a band position where it has no meaning.
    InvalidBand {
        /// Zero-based band position.
        position: usize,
        /// The color that was given.
        color: crate::Color,
        /// What the band position represents.
        role: &'static str,
    },
    /// The value cannot be shown with the requested number of digits.
    NotRepresentable(f64),
    /// The value is outside the supported range.
    OutOfRange(f64),
    /// Text could not be parsed as a resistance or code.
    Parse(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BandCount(n) => write!(f, "{n} bands is not a valid resistor code"),
            Error::InvalidBand {
                position,
                color,
                role,
            } => write!(
                f,
                "band {} cannot be {} (it is a {role} band)",
                position + 1,
                color.name()
            ),
            Error::NotRepresentable(v) => write!(
                f,
                "{} cannot be shown exactly with this many bands",
                crate::value::format_ohms(*v)
            ),
            Error::OutOfRange(v) => write!(f, "{v} Ω is outside the supported range"),
            Error::Parse(s) => write!(f, "could not understand \"{s}\""),
        }
    }
}

impl std::error::Error for Error {}
