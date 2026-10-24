use axum::http::{HeaderMap, HeaderName, header};
use std::iter;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Encoding {
    Identity,
    Gzip,
    Brotli,
}

impl Encoding {
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Identity => "",
            Self::Gzip => ".gz",
            Self::Brotli => ".br",
        }
    }

    pub fn content_encoding(self) -> Option<&'static str> {
        match self {
            Self::Identity => None,
            Self::Gzip => Some("gzip"),
            Self::Brotli => Some("br"),
        }
    }
}

pub fn accepted_encodings(headers: &HeaderMap) -> Vec<Encoding> {
    let mut qualities = [None::<u16>; 4];
    for (name, quality) in weighted_values(headers, header::ACCEPT_ENCODING) {
        let index = match name.as_str() {
            "identity" => 0,
            "gzip" | "x-gzip" => 1,
            "br" => 2,
            "*" => 3,
            _ => continue,
        };
        qualities[index] = Some(qualities[index].map_or(quality, |old| old.min(quality)));
    }

    // Unlisted identity is a fallback, unless the wildcard explicitly excludes it.
    let identity = qualities[0].unwrap_or(if qualities[3] == Some(0) { 0 } else { 1 });
    let mut candidates = [
        (identity, Encoding::Identity),
        (qualities[1].or(qualities[3]).unwrap_or(0), Encoding::Gzip),
        (qualities[2].or(qualities[3]).unwrap_or(0), Encoding::Brotli),
    ];
    candidates.sort_unstable_by(|left, right| right.cmp(left));
    candidates
        .into_iter()
        .filter_map(|(quality, encoding)| (quality > 0).then_some(encoding))
        .collect()
}

pub fn accepts_html(headers: &HeaderMap) -> bool {
    weighted_values(headers, header::ACCEPT)
        .filter(|(name, _)| name == "text/html")
        .map(|(_, quality)| quality)
        .min()
        .is_some_and(|quality| quality > 0)
}

fn split_quoted(value: &str, separator: char) -> impl Iterator<Item = &str> {
    let mut quoted = false;
    let mut escaped = false;
    value.split(move |character| match character {
        _ if escaped => {
            escaped = false;
            false
        }
        '\\' if quoted => {
            escaped = true;
            false
        }
        '"' => {
            quoted = !quoted;
            false
        }
        character => character == separator && !quoted,
    })
}

fn weighted_values(
    headers: &HeaderMap,
    name: HeaderName,
) -> impl Iterator<Item = (String, u16)> + '_ {
    headers
        .get_all(name)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| split_quoted(value, ','))
        .filter_map(|value| {
            let mut parts = split_quoted(value, ';');
            let name = parts.next()?.trim().to_ascii_lowercase();
            if name.is_empty() {
                return None;
            }
            let mut quality = None;
            for parameter in parts {
                let Some((key, value)) = parameter.trim().split_once('=') else {
                    return Some((name, 0));
                };
                if key.trim().eq_ignore_ascii_case("q") {
                    if quality.is_some() {
                        return Some((name, 0));
                    }
                    let value = value.trim();
                    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
                    let valid = matches!(whole, "0" | "1")
                        && fraction.len() <= 3
                        && fraction.bytes().all(|byte| byte.is_ascii_digit())
                        && (whole == "0" || fraction.bytes().all(|byte| byte == b'0'));
                    if !valid {
                        return Some((name, 0));
                    }
                    quality = Some(if whole == "1" {
                        1000
                    } else {
                        fraction
                            .bytes()
                            .chain(iter::repeat(b'0'))
                            .take(3)
                            .fold(0, |value, digit| value * 10 + u16::from(digit - b'0'))
                    });
                }
            }
            Some((name, quality.unwrap_or(1000)))
        })
}

#[cfg(test)]
mod tests {
    use super::{Encoding, accepted_encodings, accepts_html};
    use axum::http::{HeaderMap, HeaderValue, header};

    #[test]
    fn when_encoding_preferences_are_supplied_then_selection_should_respect_weights_and_exclusions()
    {
        use Encoding::{Brotli, Gzip, Identity};

        for (value, expected) in [
            ("", vec![Identity]),
            ("br, gzip", vec![Brotli, Gzip, Identity]),
            ("br;q=0.1, gzip;q=0.9", vec![Gzip, Brotli, Identity]),
            ("br;q=0.1, identity;q=1", vec![Identity, Brotli]),
            ("br;q=0, *;q=0.5", vec![Gzip, Identity]),
            ("*;q=0", vec![]),
            ("*;q=0, identity;q=0.5", vec![Identity]),
            ("gzip;q=0, identity;q=0", vec![]),
            ("gzip;q=1.1, identity;q=0", vec![]),
            ("BR;Q=1, x-gzip;q=0.5", vec![Brotli, Gzip, Identity]),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::ACCEPT_ENCODING,
                HeaderValue::from_str(value).unwrap(),
            );

            let encodings = accepted_encodings(&headers);

            assert_eq!(encodings, expected, "{value}");
        }
    }

    #[test]
    fn when_html_is_not_explicitly_acceptable_then_navigation_should_not_rewrite() {
        for (value, expected) in [
            ("text/html", true),
            ("application/json, text/html;q=0.5", true),
            ("text/html;q=0, */*;q=1", false),
            ("text/html;q=invalid", false),
            ("*/*", false),
            ("text/*", false),
            ("application/json; note=\"a,text/html,b\"", false),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header::ACCEPT, HeaderValue::from_str(value).unwrap());

            let acceptable = accepts_html(&headers);

            assert_eq!(acceptable, expected, "{value}");
        }
    }
}
