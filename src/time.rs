use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{CutlineError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeValue {
    millis: u64,
}

impl TimeValue {
    pub fn from_millis(millis: u64) -> Self {
        Self { millis }
    }

    pub fn millis(self) -> u64 {
        self.millis
    }

    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        if let Some(seconds) = trimmed.strip_suffix('s') {
            return parse_seconds(seconds).map(Self::from_millis);
        }

        if trimmed.contains(':') {
            return parse_colon_time(trimmed).map(Self::from_millis);
        }

        trimmed
            .parse::<u64>()
            .map(Self::from_millis)
            .map_err(|_| CutlineError::InvalidTime(trimmed.to_owned()))
    }

    pub fn display(self) -> String {
        let total_seconds = self.millis / 1000;
        let millis = self.millis % 1000;
        let seconds = total_seconds % 60;
        let total_minutes = total_seconds / 60;
        let minutes = total_minutes % 60;
        let hours = total_minutes / 60;

        format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
    }

    pub fn as_ffmpeg_seconds(self) -> String {
        format!("{}.{:03}", self.millis / 1000, self.millis % 1000)
    }
}

impl Serialize for TimeValue {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.display())
    }
}

impl<'de> Deserialize<'de> for TimeValue {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawTime {
            String(String),
            Millis(u64),
        }

        match RawTime::deserialize(deserializer)? {
            RawTime::String(value) => TimeValue::parse(&value).map_err(serde::de::Error::custom),
            RawTime::Millis(value) => Ok(TimeValue::from_millis(value)),
        }
    }
}

fn parse_seconds(input: &str) -> Result<u64> {
    let seconds = input
        .parse::<f64>()
        .map_err(|_| CutlineError::InvalidTime(format!("{input}s")))?;

    if !seconds.is_finite() || seconds < 0.0 {
        return Err(CutlineError::InvalidTime(format!("{input}s")));
    }

    Ok((seconds * 1000.0).round() as u64)
}

fn parse_colon_time(input: &str) -> Result<u64> {
    let parts: Vec<&str> = input.split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return Err(CutlineError::InvalidTime(input.to_owned()));
    }

    let (hours, minutes, seconds) = match parts.as_slice() {
        [minutes, seconds] => (0, parse_integer(minutes)?, parse_seconds_part(seconds)?),
        [hours, minutes, seconds] => (
            parse_integer(hours)?,
            parse_integer(minutes)?,
            parse_seconds_part(seconds)?,
        ),
        _ => return Err(CutlineError::InvalidTime(input.to_owned())),
    };

    if minutes >= 60 || seconds >= 60_000 {
        return Err(CutlineError::InvalidTime(input.to_owned()));
    }

    Ok(hours * 60 * 60 * 1000 + minutes * 60 * 1000 + seconds)
}

fn parse_integer(input: &str) -> Result<u64> {
    input
        .parse::<u64>()
        .map_err(|_| CutlineError::InvalidTime(input.to_owned()))
}

fn parse_seconds_part(input: &str) -> Result<u64> {
    let (seconds, millis) = match input.split_once('.') {
        Some((seconds, fraction)) => {
            let mut padded = fraction.to_owned();
            padded.truncate(3);
            while padded.len() < 3 {
                padded.push('0');
            }
            (parse_integer(seconds)?, parse_integer(&padded)?)
        }
        None => (parse_integer(input)?, 0),
    };

    Ok(seconds * 1000 + millis)
}

#[cfg(test)]
mod tests {
    use super::TimeValue;

    #[test]
    fn parses_supported_time_values() {
        assert_eq!(TimeValue::parse("12:34").unwrap().millis(), 754_000);
        assert_eq!(TimeValue::parse("01:12:34").unwrap().millis(), 4_354_000);
        assert_eq!(
            TimeValue::parse("01:12:34.567").unwrap().millis(),
            4_354_567
        );
        assert_eq!(TimeValue::parse("754.2s").unwrap().millis(), 754_200);
        assert_eq!(TimeValue::parse("754200").unwrap().millis(), 754_200);
    }

    #[test]
    fn displays_canonical_time() {
        assert_eq!(TimeValue::from_millis(4_354_567).display(), "01:12:34.567");
    }

    #[test]
    fn formats_ffmpeg_seconds() {
        assert_eq!(
            TimeValue::from_millis(4_354_567).as_ffmpeg_seconds(),
            "4354.567"
        );
    }

    #[test]
    fn rejects_out_of_range_colon_parts() {
        assert!(TimeValue::parse("01:60:00").is_err());
        assert!(TimeValue::parse("01:00:60").is_err());
    }
}
