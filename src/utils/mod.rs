use actix_web::HttpRequest;
use rand::Rng;
use regex::Regex;
use rust_decimal::Decimal;
use std::sync::OnceLock;

/// Extract the preferred language from the `Accept-Language` header.
///
/// Returns a two-letter language code (e.g. `"pl"`, `"en"`).
/// Defaults to `"en"` when the header is missing or cannot be parsed.
pub fn extract_lang(req: &HttpRequest) -> String {
    req.headers()
        .get("Accept-Language")
        .and_then(|v| v.to_str().ok())
        .and_then(|header| {
            // Accept-Language can look like "pl,en-US;q=0.9,en;q=0.8"
            // Take the first tag and extract the primary language subtag.
            header
                .split(',')
                .next()
                .map(|tag| tag.split(';').next().unwrap_or(tag))
                .map(|tag| tag.split('-').next().unwrap_or(tag))
                .map(|lang| lang.trim().to_lowercase())
        })
        .filter(|lang| !lang.is_empty())
        .unwrap_or_else(|| "en".to_string())
}

/// Calculate the great-circle distance between two geographic coordinates
/// using the Haversine formula.
///
/// Returns the distance in **kilometres**.
pub fn calculate_distance(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6_371.0;

    let d_lat = (lat2 - lat1).to_radians();
    let d_lng = (lng2 - lng1).to_radians();

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();

    let a = (d_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (d_lng / 2.0).sin().powi(2);

    let c = 2.0 * a.sqrt().asin();

    EARTH_RADIUS_KM * c
}

/// Generate a random numeric code of the given length (e.g. for SMS verification).
///
/// # Example
/// ```ignore
/// let code = generate_random_code(6); // e.g. "048291"
/// ```
pub fn generate_random_code(length: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| rng.gen_range(0..10).to_string())
        .collect()
}

/// Basic XSS prevention: strip `<script>` tags, HTML event attributes,
/// and common dangerous patterns from the input.
pub fn sanitize_string(input: &str) -> String {
    static SCRIPT_RE: OnceLock<Regex> = OnceLock::new();
    static EVENT_RE: OnceLock<Regex> = OnceLock::new();
    static TAG_RE: OnceLock<Regex> = OnceLock::new();

    let script_re = SCRIPT_RE.get_or_init(|| {
        Regex::new(r"(?i)<script[^>]*>[\s\S]*?</script>").expect("invalid regex")
    });

    let event_re = EVENT_RE.get_or_init(|| {
        Regex::new(r#"(?i)\s+on\w+\s*=\s*["'][^"']*["']"#).expect("invalid regex")
    });

    let tag_re = TAG_RE.get_or_init(|| {
        Regex::new(r"<[^>]+>").expect("invalid regex")
    });

    let result = script_re.replace_all(input, "");
    let result = event_re.replace_all(&result, "");
    let result = tag_re.replace_all(&result, "");

    result
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Create a URL-safe slug from an arbitrary string.
///
/// Converts to lowercase, replaces non-alphanumeric characters with hyphens,
/// collapses consecutive hyphens, and trims leading/trailing hyphens.
pub fn slug_from_string(input: &str) -> String {
    static NON_ALNUM_RE: OnceLock<Regex> = OnceLock::new();
    static MULTI_HYPHEN_RE: OnceLock<Regex> = OnceLock::new();

    let non_alnum_re =
        NON_ALNUM_RE.get_or_init(|| Regex::new(r"[^a-z0-9]+").expect("invalid regex"));
    let multi_hyphen_re =
        MULTI_HYPHEN_RE.get_or_init(|| Regex::new(r"-{2,}").expect("invalid regex"));

    let slug = input.to_lowercase();
    let slug = non_alnum_re.replace_all(&slug, "-");
    let slug = multi_hyphen_re.replace_all(&slug, "-");

    slug.trim_matches('-').to_string()
}

/// Validate an ISBN-10 or ISBN-13 string.
///
/// Accepts both hyphenated and non-hyphenated forms.
pub fn validate_isbn(isbn: &str) -> bool {
    let digits: String = isbn.chars().filter(|c| c.is_ascii_alphanumeric()).collect();

    match digits.len() {
        10 => validate_isbn10(&digits),
        13 => validate_isbn13(&digits),
        _ => false,
    }
}

fn validate_isbn10(digits: &str) -> bool {
    let chars: Vec<char> = digits.chars().collect();
    let mut sum: u32 = 0;

    for (i, ch) in chars.iter().enumerate() {
        let value = if i == 9 && (*ch == 'X' || *ch == 'x') {
            10
        } else if let Some(d) = ch.to_digit(10) {
            d
        } else {
            return false;
        };
        sum += value * (10 - i as u32);
    }

    sum % 11 == 0
}

fn validate_isbn13(digits: &str) -> bool {
    let mut sum: u32 = 0;

    for (i, ch) in digits.chars().enumerate() {
        let d = match ch.to_digit(10) {
            Some(d) => d,
            None => return false,
        };
        let weight = if i % 2 == 0 { 1 } else { 3 };
        sum += d * weight;
    }

    sum % 10 == 0
}

/// Format a `Decimal` amount as a currency string with two decimal places
/// and the PLN symbol.
///
/// # Example
/// ```ignore
/// use rust_decimal_macros::dec;
/// assert_eq!(format_currency(dec!(12.5)), "12.50 PLN");
/// ```
pub fn format_currency(amount: Decimal) -> String {
    format!("{:.2} PLN", amount)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_calculate_distance_same_point() {
        let d = calculate_distance(52.2297, 21.0122, 52.2297, 21.0122);
        assert!((d - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_distance_warsaw_krakow() {
        // Warsaw -> Krakow ~252 km
        let d = calculate_distance(52.2297, 21.0122, 50.0647, 19.9450);
        assert!(d > 240.0 && d < 260.0);
    }

    #[test]
    fn test_generate_random_code_length() {
        let code = generate_random_code(6);
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_sanitize_string_strips_script() {
        let input = "Hello<script>alert('xss')</script>World";
        let output = sanitize_string(input);
        assert!(!output.contains("<script>"));
        assert!(!output.contains("alert"));
    }

    #[test]
    fn test_slug_from_string() {
        assert_eq!(slug_from_string("Hello World!"), "hello-world");
        assert_eq!(slug_from_string("  Some--Title  "), "some-title");
        assert_eq!(slug_from_string("Książka po polsku"), "ksi-ka-po-polsku");
    }

    #[test]
    fn test_validate_isbn13_valid() {
        assert!(validate_isbn("978-3-16-148410-0"));
    }

    #[test]
    fn test_validate_isbn10_valid() {
        assert!(validate_isbn("0-306-40615-2"));
    }

    #[test]
    fn test_validate_isbn_invalid() {
        assert!(!validate_isbn("1234567890"));
        assert!(!validate_isbn("not-an-isbn"));
    }

    #[test]
    fn test_format_currency() {
        assert_eq!(format_currency(dec!(12.5)), "12.50 PLN");
        assert_eq!(format_currency(dec!(0)), "0.00 PLN");
        assert_eq!(format_currency(dec!(99.99)), "99.99 PLN");
    }
}
