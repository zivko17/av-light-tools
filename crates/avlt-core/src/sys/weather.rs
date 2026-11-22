//! `util weather` request (Weather.ps1): wttr.in in English, metric units.

/// Output style of wttr.in (Weather.ps1 `-Format`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherFormat {
    /// Three-day forecast.
    Full,
    /// `format=v2` data-rich report (Weather.ps1 default).
    Short,
    /// One line: place, condition, temperature, wind, humidity.
    Mini,
}

/// wttr.in URL for `city` (percent-encoded) in `format`. Without `color` the `T` option
/// asks for plain text (no terminal escape sequences).
pub fn weather_url(city: &str, format: WeatherFormat, color: bool) -> String {
    let city = encode_path(city.trim());
    let mut url = format!("https://wttr.in/{city}?lang=en&M");
    match format {
        WeatherFormat::Full => {}
        WeatherFormat::Short => url.push_str("&format=v2"),
        WeatherFormat::Mini => url.push_str("&format=%l:+%c+%t+%w+%h"),
    }
    if !color {
        url.push_str("&T");
    }
    url
}

/// Percent-encodes everything but unreserved characters (RFC 3986), UTF-8 based.
fn encode_path(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_match_weather_ps1() {
        assert_eq!(
            weather_url("Madrid", WeatherFormat::Full, true),
            "https://wttr.in/Madrid?lang=en&M"
        );
        assert_eq!(
            weather_url("Madrid", WeatherFormat::Short, true),
            "https://wttr.in/Madrid?lang=en&M&format=v2"
        );
        assert_eq!(
            weather_url("Madrid", WeatherFormat::Mini, true),
            "https://wttr.in/Madrid?lang=en&M&format=%l:+%c+%t+%w+%h"
        );
    }

    #[test]
    fn city_is_encoded_and_plain_text_requested_without_colour() {
        assert_eq!(
            weather_url(" Palma de Mallorca ", WeatherFormat::Mini, false),
            "https://wttr.in/Palma%20de%20Mallorca?lang=en&M&format=%l:+%c+%t+%w+%h&T"
        );
        assert_eq!(
            weather_url("A Coruña", WeatherFormat::Full, false),
            "https://wttr.in/A%20Coru%C3%B1a?lang=en&M&T"
        );
        assert_eq!(
            weather_url("x/../y?z", WeatherFormat::Full, true),
            "https://wttr.in/x%2F..%2Fy%3Fz?lang=en&M"
        );
    }
}
