use media_stream::{parse_range_header, resolve_range, ParsedRange, RangeError};

#[test]
fn test_bounded_range_standard() {
    let parsed = parse_range_header("bytes=0-499").expect("Valid bounded range");
    assert_eq!(parsed, ParsedRange::Bounded { start: 0, end: 499 });

    let resolved = resolve_range(parsed, 10_000).expect("Resolves within bounds");
    assert_eq!(resolved.start, 0);
    assert_eq!(resolved.end, 499);
    assert_eq!(resolved.content_length(), 500);
    assert_eq!(resolved.content_range_header(), "bytes 0-499/10000");
}

#[test]
fn test_bounded_range_clamp_eof() {
    let parsed = parse_range_header("bytes=9500-15000").expect("Valid bounded range");
    let resolved = resolve_range(parsed, 10_000).expect("Clamps to EOF");
    assert_eq!(resolved.start, 9500);
    assert_eq!(resolved.end, 9999);
    assert_eq!(resolved.content_length(), 500);
    assert_eq!(resolved.content_range_header(), "bytes 9500-9999/10000");
}

#[test]
fn test_open_range() {
    let parsed = parse_range_header("bytes=5000-").expect("Valid open range");
    assert_eq!(parsed, ParsedRange::Open { start: 5000 });

    let resolved = resolve_range(parsed, 10_000).expect("Resolves open range");
    assert_eq!(resolved.start, 5000);
    assert_eq!(resolved.end, 9999);
    assert_eq!(resolved.content_length(), 5000);
    assert_eq!(resolved.content_range_header(), "bytes 5000-9999/10000");
}

#[test]
fn test_suffix_range_standard() {
    let parsed = parse_range_header("bytes=-500").expect("Valid suffix range");
    assert_eq!(parsed, ParsedRange::Suffix { suffix: 500 });

    let resolved = resolve_range(parsed, 10_000).expect("Resolves suffix range");
    assert_eq!(resolved.start, 9500);
    assert_eq!(resolved.end, 9999);
    assert_eq!(resolved.content_length(), 500);
    assert_eq!(resolved.content_range_header(), "bytes 9500-9999/10000");
}

#[test]
fn test_suffix_range_larger_than_file() {
    let parsed = parse_range_header("bytes=-20000").expect("Valid suffix range");
    let resolved = resolve_range(parsed, 10_000).expect("Resolves whole file");
    assert_eq!(resolved.start, 0);
    assert_eq!(resolved.end, 9999);
    assert_eq!(resolved.content_length(), 10_000);
}

#[test]
fn test_single_byte_boundaries() {
    // First byte
    let p1 = parse_range_header("bytes=0-0").unwrap();
    let r1 = resolve_range(p1, 10_000).unwrap();
    assert_eq!(r1.start, 0);
    assert_eq!(r1.end, 0);
    assert_eq!(r1.content_length(), 1);

    // Last byte
    let p2 = parse_range_header("bytes=9999-9999").unwrap();
    let r2 = resolve_range(p2, 10_000).unwrap();
    assert_eq!(r2.start, 9999);
    assert_eq!(r2.end, 9999);
    assert_eq!(r2.content_length(), 1);
}

#[test]
fn test_unsatisfiable_inverted() {
    let err = parse_range_header("bytes=500-400");
    assert!(matches!(err, Err(RangeError::Unsatisfiable { .. })));
}

#[test]
fn test_unsatisfiable_start_equals_or_exceeds_size() {
    let p1 = parse_range_header("bytes=10000-").unwrap();
    assert!(matches!(
        resolve_range(p1, 10_000),
        Err(RangeError::Unsatisfiable { total_size: 10_000 })
    ));

    let p2 = parse_range_header("bytes=15000-20000").unwrap();
    assert!(matches!(
        resolve_range(p2, 10_000),
        Err(RangeError::Unsatisfiable { total_size: 10_000 })
    ));
}

#[test]
fn test_unsatisfiable_zero_byte_file() {
    let p = parse_range_header("bytes=0-").unwrap();
    assert!(matches!(
        resolve_range(p, 0),
        Err(RangeError::Unsatisfiable { total_size: 0 })
    ));
}

#[test]
fn test_unsatisfiable_zero_suffix() {
    let err = parse_range_header("bytes=-0");
    assert!(matches!(err, Err(RangeError::MalformedHeader(_))));
}

#[test]
fn test_malformed_headers() {
    assert!(matches!(parse_range_header("bytes="), Err(RangeError::MalformedHeader(_))));
    assert!(matches!(parse_range_header("bytes=abc-def"), Err(RangeError::MalformedHeader(_))));
    assert!(matches!(parse_range_header("bytes=500"), Err(RangeError::MalformedHeader(_))));
    assert!(matches!(parse_range_header("items=0-100"), Err(RangeError::InvalidUnit)));
    assert!(matches!(parse_range_header("bytes=0-10, 20-30"), Err(RangeError::MultipleRangesUnsupported)));
}
