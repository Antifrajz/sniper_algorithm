use core::fmt;
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub enum TIF {
    GTC, // Good-Till-Cancel
    IOC, // Immediate-Or-Cancel
    FOK, // Fill-Or-Kill
    GTX, // Good-Till-Crossing (Post Only)
    GTD, // Good-Till-Date
}

impl TIF {
    pub fn to_int(&self) -> i32 {
        match self {
            TIF::GTC => 1,
            TIF::IOC => 2,
            TIF::FOK => 3,
            TIF::GTX => 4,
            TIF::GTD => 5,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            TIF::GTC => "GTC",
            TIF::IOC => "IOC",
            TIF::FOK => "FOK",
            TIF::GTX => "GTX",
            TIF::GTD => "GTD",
        }
    }
}

impl fmt::Display for TIF {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

impl FromStr for TIF {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GTC" => Ok(TIF::GTC),
            "IOC" => Ok(TIF::IOC),
            "FOK" => Ok(TIF::FOK),
            "GTX" => Ok(TIF::GTX),
            "GTD" => Ok(TIF::GTD),
            _ => Err(format!("'{}' is not a valid TIF value", s)),
        }
    }
}
