use crate::Error;
use std::fmt;
use std::str::FromStr;

/// Optional byte-range selection and its upstream representation validator.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaOptions<'a> {
    pub range: Option<MediaRange>,
    pub if_range: Option<&'a str>,
}

/// One HTTP byte range, without multipart or other range units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRange(String);

impl FromStr for MediaRange {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = || Error::InvalidRequest {
            field: "media_range",
        };
        let (start, end) = value
            .strip_prefix("bytes=")
            .and_then(|value| value.split_once('-'))
            .ok_or_else(invalid)?;
        let mut bounds = [None, None];
        for (index, value) in [start, end].into_iter().enumerate() {
            if !value.is_empty() {
                if !value.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Err(invalid());
                }
                bounds[index] = Some(value.parse::<u64>().map_err(|_| invalid())?);
            }
        }
        match bounds {
            [None, None] => return Err(invalid()),
            [Some(start), Some(end)] if start > end => return Err(invalid()),
            _ => {}
        }
        Ok(Self(value.to_owned()))
    }
}

impl fmt::Display for MediaRange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_a_range_is_parsed_then_only_single_byte_ranges_should_be_accepted() {
        for value in [
            "bytes=0-1",
            "bytes=12-",
            "bytes=-32",
            "bytes=-0",
            "bytes=00-01",
        ] {
            let range: MediaRange = value.parse().unwrap();

            assert_eq!(range.to_string(), value);
        }
        for value in [
            "",
            "bytes=-",
            "bytes=2-1",
            "bytes=0-1,2-3",
            "items=0-1",
            "bytes=+1-2",
            "bytes=0- 1",
            "bytes=0--1",
            "bytes=18446744073709551616-",
            "bytes=0-1\r\nCookie: secret",
        ] {
            let result = value.parse::<MediaRange>();

            assert!(
                matches!(
                    result,
                    Err(Error::InvalidRequest {
                        field: "media_range"
                    })
                ),
                "{value}"
            );
        }
    }
}
