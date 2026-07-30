//! Pure parsers for the Linux `/sys` and `/proc` text formats used by
//! [`super::linux`]. Kept free of `std::fs` so they're unit-testable on any
//! host, independent of whether the `linux` backend itself is compiled.

/// Parses the content of a `.../topology/physical_package_id` file.
pub(crate) fn parse_physical_package_id(s: &str) -> Option<usize> {
    s.trim().parse().ok()
}

/// Parses the content of a `.../cpufreq/cpuinfo_max_freq` file (kHz).
pub(crate) fn parse_cpuinfo_max_freq(s: &str) -> Option<u64> {
    s.trim().parse().ok()
}

/// Parses the `MemTotal:` line out of `/proc/meminfo` or a
/// `/sys/devices/system/node/nodeN/meminfo` file (the latter prefixes every
/// line with `Node N `), returning the value in kB.
pub(crate) fn parse_mem_total_kb(s: &str) -> Option<u64> {
    s.lines()
        .find_map(|line| line.split_once("MemTotal:"))
        .and_then(|(_, rest)| rest.trim().split_whitespace().next())
        .and_then(|n| n.parse().ok())
}

/// Parses the content of a `.../cache/indexN/level` file (1, 2, 3, ...).
pub(crate) fn parse_cache_level(s: &str) -> Option<u8> {
    s.trim().parse().ok()
}

/// Parses the content of a `.../cache/indexN/size` file, such as `"32K"` or
/// `"1024K"`, into a byte count. A bare number (no suffix) is taken as bytes.
pub(crate) fn parse_cache_size(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (digits, multiplier) = match s.chars().last() {
        Some(c) if c.eq_ignore_ascii_case(&'k') => (&s[..s.len() - 1], 1024),
        Some(c) if c.eq_ignore_ascii_case(&'m') => (&s[..s.len() - 1], 1024 * 1024),
        Some(c) if c.eq_ignore_ascii_case(&'g') => (&s[..s.len() - 1], 1024 * 1024 * 1024),
        _ => (s, 1),
    };
    digits.trim().parse::<u64>().ok().map(|n| n * multiplier)
}

/// True for a `.../cache/indexN/type` content of `"Data"` or `"Unified"`.
/// Used to skip pure instruction caches, since [`super::HardwareDataKind`]'s
/// `L1`/etc. only hold a single size (the data/unified cache is reported).
pub(crate) fn is_data_or_unified_cache_type(s: &str) -> bool {
    matches!(s.trim(), "Data" | "Unified")
}

/// Extracts the value after a `"Label:"` prefix from a `/proc/[pid]/status`
/// line, trimmed of the tab/spaces `proc` pads it with. Returns `None` if no
/// line starts with that exact label.
pub(crate) fn parse_status_field<'a>(content: &'a str, label: &str) -> Option<&'a str> {
    content
        .lines()
        .find_map(|line| line.strip_prefix(label))
        .map(str::trim)
}

/// Parses a Linux cpu list such as `"0-3,8,9-11"` into individual cpu ids.
pub(crate) fn parse_cpulist(s: &str) -> Vec<usize> {
    let mut cpus = Vec::new();
    for part in s.trim().split(',').filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((a, b)) => {
                if let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>()) {
                    cpus.extend(a..=b);
                }
            }
            None => {
                if let Ok(c) = part.parse::<usize>() {
                    cpus.push(c);
                }
            }
        }
    }
    cpus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_physical_package_id() {
        assert_eq!(parse_physical_package_id("0\n"), Some(0));
        assert_eq!(parse_physical_package_id("1"), Some(1));
        assert_eq!(parse_physical_package_id("oops"), None);
    }

    #[test]
    fn parses_cpuinfo_max_freq() {
        assert_eq!(parse_cpuinfo_max_freq("3800000\n"), Some(3_800_000));
        assert_eq!(parse_cpuinfo_max_freq(""), None);
    }

    #[test]
    fn parses_mem_total_from_proc_meminfo() {
        let content = "MemTotal:       16336864 kB\nMemFree:         1234 kB\n";
        assert_eq!(parse_mem_total_kb(content), Some(16_336_864));
    }

    #[test]
    fn parses_mem_total_from_node_meminfo() {
        let content = "Node 0 MemTotal:       8168432 kB\nNode 0 MemFree:        123 kB\n";
        assert_eq!(parse_mem_total_kb(content), Some(8_168_432));
    }

    #[test]
    fn parses_mem_total_missing() {
        assert_eq!(parse_mem_total_kb("MemFree: 123 kB\n"), None);
    }

    #[test]
    fn parses_status_field() {
        let content = "Name:\tbash\nCpus_allowed_list:\t0-3,8\nMems_allowed_list:\t0\n";
        assert_eq!(
            parse_status_field(content, "Cpus_allowed_list:"),
            Some("0-3,8")
        );
        assert_eq!(parse_status_field(content, "Mems_allowed_list:"), Some("0"));
        assert_eq!(parse_status_field(content, "Nonexistent:"), None);
    }

    #[test]
    fn parses_cpulist_ranges_and_singletons() {
        assert_eq!(parse_cpulist("0-3,8,9-11"), vec![0, 1, 2, 3, 8, 9, 10, 11]);
        assert_eq!(parse_cpulist("5"), vec![5]);
        assert_eq!(parse_cpulist(""), Vec::<usize>::new());
    }

    #[test]
    fn parses_cache_level() {
        assert_eq!(parse_cache_level("1\n"), Some(1));
        assert_eq!(parse_cache_level("3"), Some(3));
        assert_eq!(parse_cache_level("oops"), None);
    }

    #[test]
    fn parses_cache_size_with_suffixes() {
        assert_eq!(parse_cache_size("32K\n"), Some(32 * 1024));
        assert_eq!(parse_cache_size("1024K"), Some(1024 * 1024));
        assert_eq!(parse_cache_size("8M"), Some(8 * 1024 * 1024));
        assert_eq!(parse_cache_size("1G"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_cache_size("12288"), Some(12288));
        assert_eq!(parse_cache_size(""), None);
    }

    #[test]
    fn recognizes_data_and_unified_cache_types() {
        assert!(is_data_or_unified_cache_type("Data"));
        assert!(is_data_or_unified_cache_type("Unified"));
        assert!(is_data_or_unified_cache_type(" Data \n"));
        assert!(!is_data_or_unified_cache_type("Instruction"));
    }
}
