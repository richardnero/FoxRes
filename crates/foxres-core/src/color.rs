//! IEC 60062 resistor color table.

/// A band color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    Pink,
    Silver,
    Gold,
    Black,
    Brown,
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Violet,
    Grey,
    White,
    /// No band (only valid as the tolerance band, meaning ±20 %).
    None,
}

impl Color {
    /// Every color, in the order they are usually listed.
    pub const ALL: [Color; 14] = [
        Color::Black,
        Color::Brown,
        Color::Red,
        Color::Orange,
        Color::Yellow,
        Color::Green,
        Color::Blue,
        Color::Violet,
        Color::Grey,
        Color::White,
        Color::Gold,
        Color::Silver,
        Color::Pink,
        Color::None,
    ];

    /// Colors that have a significant-digit value, `0..=9`.
    pub const DIGITS: [Color; 10] = [
        Color::Black,
        Color::Brown,
        Color::Red,
        Color::Orange,
        Color::Yellow,
        Color::Green,
        Color::Blue,
        Color::Violet,
        Color::Grey,
        Color::White,
    ];

    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            Color::Pink => "Pink",
            Color::Silver => "Silver",
            Color::Gold => "Gold",
            Color::Black => "Black",
            Color::Brown => "Brown",
            Color::Red => "Red",
            Color::Orange => "Orange",
            Color::Yellow => "Yellow",
            Color::Green => "Green",
            Color::Blue => "Blue",
            Color::Violet => "Violet",
            Color::Grey => "Grey",
            Color::White => "White",
            Color::None => "None",
        }
    }

    /// Significant digit, if this color has one.
    pub fn digit(self) -> Option<u8> {
        Self::DIGITS
            .iter()
            .position(|&c| c == self)
            .map(|i| i as u8)
    }

    /// Color for a significant digit `0..=9`.
    pub fn from_digit(d: u8) -> Option<Color> {
        Self::DIGITS.get(usize::from(d)).copied()
    }

    /// Power of ten applied by the multiplier band.
    pub fn multiplier_exp(self) -> Option<i32> {
        match self {
            Color::Pink => Some(-3),
            Color::Silver => Some(-2),
            Color::Gold => Some(-1),
            Color::None => None,
            c => c.digit().map(i32::from),
        }
    }

    /// Color for a multiplier exponent `-3..=9`.
    pub fn from_multiplier_exp(e: i32) -> Option<Color> {
        match e {
            -3 => Some(Color::Pink),
            -2 => Some(Color::Silver),
            -1 => Some(Color::Gold),
            0..=9 => Color::from_digit(e as u8),
            _ => None,
        }
    }

    /// Tolerance in percent.
    pub fn tolerance(self) -> Option<f64> {
        match self {
            Color::Brown => Some(1.0),
            Color::Red => Some(2.0),
            Color::Orange => Some(0.05),
            Color::Yellow => Some(0.02),
            Color::Green => Some(0.5),
            Color::Blue => Some(0.25),
            Color::Violet => Some(0.1),
            Color::Grey => Some(0.01),
            Color::Gold => Some(5.0),
            Color::Silver => Some(10.0),
            Color::None => Some(20.0),
            _ => None,
        }
    }

    /// Temperature coefficient in ppm/K.
    pub fn tempco(self) -> Option<u32> {
        match self {
            Color::Black => Some(250),
            Color::Brown => Some(100),
            Color::Red => Some(50),
            Color::Orange => Some(15),
            Color::Yellow => Some(25),
            Color::Green => Some(20),
            Color::Blue => Some(10),
            Color::Violet => Some(5),
            Color::Grey => Some(1),
            _ => None,
        }
    }

    /// sRGB color used to draw the band.
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            Color::Pink => (0xFF, 0x69, 0xB4),
            Color::Silver => (0xC0, 0xC0, 0xC0),
            Color::Gold => (0xCF, 0xB5, 0x3B),
            Color::Black => (0x1A, 0x1A, 0x1A),
            Color::Brown => (0x8B, 0x45, 0x13),
            Color::Red => (0xD0, 0x1C, 0x1C),
            Color::Orange => (0xFF, 0x8C, 0x00),
            Color::Yellow => (0xFF, 0xD7, 0x00),
            Color::Green => (0x2E, 0x8B, 0x22),
            Color::Blue => (0x1E, 0x5A, 0xD6),
            Color::Violet => (0x8A, 0x2B, 0xE2),
            Color::Grey => (0x80, 0x80, 0x80),
            Color::White => (0xF5, 0xF5, 0xF5),
            Color::None => (0xE3, 0xC9, 0x9A),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_round_trip() {
        for d in 0..=9 {
            assert_eq!(Color::from_digit(d).unwrap().digit(), Some(d));
        }
        assert_eq!(Color::from_digit(10), None);
    }

    #[test]
    fn multipliers_round_trip() {
        for e in -3..=9 {
            assert_eq!(
                Color::from_multiplier_exp(e).unwrap().multiplier_exp(),
                Some(e)
            );
        }
        assert_eq!(Color::from_multiplier_exp(10), None);
    }

    #[test]
    fn tolerance_table() {
        assert_eq!(Color::Gold.tolerance(), Some(5.0));
        assert_eq!(Color::Brown.tolerance(), Some(1.0));
        assert_eq!(Color::Black.tolerance(), None);
    }
}
