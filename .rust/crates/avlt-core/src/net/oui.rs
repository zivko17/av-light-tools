//! MAC address helpers and the embedded OUI vendor table.

pub use super::oui_table::OUI_TABLE;

/// The 24-bit OUI (first three bytes) of a MAC address written in any common style:
/// `AA-BB-CC-DD-EE-FF`, `aa:bb:cc:dd:ee:ff`, `aabb.ccdd.eeff`, `AABBCCDDEEFF`, or the
/// bare six-digit OUI. Returns `None` for anything else.
pub fn oui_prefix(mac: &str) -> Option<u32> {
    let mut digits = String::with_capacity(12);
    for c in mac.trim().chars() {
        match c {
            '-' | ':' | '.' => {}
            c if c.is_ascii_hexdigit() => digits.push(c),
            _ => return None,
        }
    }
    if digits.len() != 6 && digits.len() != 12 {
        return None;
    }
    u32::from_str_radix(&digits[..6], 16).ok()
}

/// Vendor of a MAC address from the embedded [`OUI_TABLE`] (any format, see [`oui_prefix`]).
pub fn oui_vendor(mac: &str) -> Option<&'static str> {
    let prefix = oui_prefix(mac)?;
    OUI_TABLE
        .binary_search_by_key(&prefix, |(p, _)| *p)
        .ok()
        .map(|i| OUI_TABLE[i].1)
}

/// `AA-BB-CC-DD-EE-FF` (the style of `Get-NetAdapter` / `Get-NetNeighbor`), or `None`
/// for an empty or all-zero address.
pub fn format_mac(bytes: &[u8]) -> Option<String> {
    if bytes.iter().all(|b| *b == 0) {
        return None;
    }
    Some(
        bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join("-"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_cases() {
        assert_eq!(oui_vendor("zz"), None);
        assert_eq!(
            oui_vendor("AA-BB-CC-00-11-22"),
            oui_vendor("aa:bb:cc:00:11:22")
        );
    }

    #[test]
    fn known_prefixes_any_format() {
        assert_eq!(oui_vendor("7C-2E-0D-12-34-56"), Some("Blackmagic Design"));
        assert_eq!(oui_vendor("7c:2e:0d:12:34:56"), Some("Blackmagic Design"));
        assert_eq!(oui_vendor("7c2e.0d12.3456"), Some("Blackmagic Design"));
        assert_eq!(oui_vendor("7C2E0D"), Some("Blackmagic Design"));
        assert_eq!(oui_vendor("00-1D-C1-00-00-01"), Some("Audinate (Dante)"));
        assert_eq!(oui_vendor("00:E0:4C:68:00:01"), Some("Realtek"));
        assert_eq!(oui_vendor("54-B5-6C-00-00-00"), Some("NovaStar"));
        assert_eq!(oui_vendor("00-00-0C-07-AC-01"), Some("Cisco"));
    }

    #[test]
    fn unknown_or_malformed() {
        assert_eq!(oui_vendor(""), None);
        assert_eq!(oui_vendor("-"), None);
        assert_eq!(oui_vendor("AA-BB-C"), None);
        assert_eq!(oui_vendor("GG-BB-CC-00-11-22"), None);
        assert_eq!(oui_vendor("7C-2E-0D-12-34-5"), None, "11 hex digits");
        assert_eq!(oui_vendor("7C-2E-0D-12-34-56-78"), None, "too long");
        assert_eq!(oui_vendor("02-00-00-00-00-00"), None, "not in table");
    }

    #[test]
    fn prefix_parsing() {
        assert_eq!(oui_prefix("7c:2e:0d:12:34:56"), Some(0x7C2E0D));
        assert_eq!(oui_prefix(" 7C2E0D "), Some(0x7C2E0D));
        assert_eq!(oui_prefix("zz"), None);
    }

    #[test]
    fn table_is_sorted_and_unique() {
        assert!(OUI_TABLE.len() > 4000);
        assert!(OUI_TABLE.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(
            OUI_TABLE
                .iter()
                .all(|(p, v)| *p <= 0xFF_FFFF && !v.is_empty())
        );
    }

    #[test]
    fn mac_formatting() {
        assert_eq!(
            format_mac(&[0x7c, 0x2e, 0x0d, 0x12, 0x34, 0x56]).as_deref(),
            Some("7C-2E-0D-12-34-56")
        );
        assert_eq!(format_mac(&[]), None);
        assert_eq!(format_mac(&[0; 6]), None, "all-zero MAC means none");
    }
}
