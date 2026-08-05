//! FSOT trinary substrate — maps continuous metrics to {-1, 0, +1}.
//! Thresholds come from preregistered `fold` constants (not ad-hoc).
#![allow(dead_code)]

use crate::fold::{S_TRIT_HI, S_TRIT_LO, TEMP_TRIT_BAND_C, TRIT_FRAC_HI, TRIT_FRAC_LO};

/// Trit represents the three-valued FSOT logic domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trit {
    Neg = -1,
    Zero = 0,
    Pos = 1,
}

impl Trit {
    pub fn as_i8(self) -> i8 {
        self as i8
    }

    pub fn from_scalar(s: f64, lo: f64, hi: f64) -> Trit {
        if s < lo {
            Trit::Neg
        } else if s > hi {
            Trit::Pos
        } else {
            Trit::Zero
        }
    }

    pub fn pair(self, other: Trit) -> Trit {
        let product = self.as_i8() * other.as_i8();
        if product > 0 {
            Trit::Pos
        } else if product < 0 {
            Trit::Neg
        } else {
            Trit::Zero
        }
    }

    pub fn neg(self) -> Trit {
        match self {
            Trit::Pos => Trit::Neg,
            Trit::Neg => Trit::Pos,
            Trit::Zero => Trit::Zero,
        }
    }

    pub fn sum_sat(self, other: Trit) -> Trit {
        let sum = self.as_i8() + other.as_i8();
        if sum > 0 {
            Trit::Pos
        } else if sum < 0 {
            Trit::Neg
        } else {
            Trit::Zero
        }
    }

    pub fn consensus(self, other: Trit) -> Trit {
        if self == other {
            self
        } else {
            Trit::Zero
        }
    }
}

/// Capacity fraction [0,1] → trit via preregistered thirds.
pub fn from_fraction(frac: f64) -> Trit {
    Trit::from_scalar(frac, TRIT_FRAC_LO, TRIT_FRAC_HI)
}

/// Temperature delta (°C) → trit via preregistered band (±e °C).
pub fn temp_delta_to_trit(delta: f64) -> Trit {
    Trit::from_scalar(delta, -TEMP_TRIT_BAND_C, TEMP_TRIT_BAND_C)
}

/// Scalar S → trit via preregistered ±K deadzone.
pub fn s_to_trit(s: f64) -> Trit {
    Trit::from_scalar(s, S_TRIT_LO, S_TRIT_HI)
}
