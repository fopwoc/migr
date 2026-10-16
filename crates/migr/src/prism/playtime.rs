use std::{fmt, ops::Add};

/// Time played in an instance, as Prism counts it in `totalTimePlayed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Playtime(u64);

impl Playtime {
    pub fn from_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    pub fn seconds(self) -> u64 {
        self.0
    }
}

impl Add for Playtime {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }
}

impl fmt::Display for Playtime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let minutes = self.0 / 60;
        write!(f, "{}h {:02}m", minutes / 60, minutes % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_hours_and_minutes() {
        assert_eq!(
            Playtime::from_seconds(36_000_000 + 59).to_string(),
            "10000h 00m"
        );
        assert_eq!(Playtime::from_seconds(3_900).to_string(), "1h 05m");
    }
}
