use qindex::models::{get_venue_tier, normalize_author_name};

#[test]
fn test_normalize_author_name() {
    assert_eq!(normalize_author_name("John Doe"), "John Doe");
    assert_eq!(normalize_author_name("Doe, John"), "John Doe");
    assert_eq!(normalize_author_name("john doe"), "John Doe");
    assert_eq!(normalize_author_name("J. Doe"), "J Doe");
    assert_eq!(normalize_author_name("  John   Doe  "), "John Doe");
}

#[test]
fn test_get_venue_tier() {
    assert_eq!(get_venue_tier("SOSP"), "A*");
    assert_eq!(get_venue_tier("sosp"), "A*");
    assert_eq!(get_venue_tier("OSDI"), "A*");
    assert_eq!(get_venue_tier("SIGMOD"), "A*");
    assert_eq!(get_venue_tier("SoCC"), "A");
    assert_eq!(get_venue_tier("Workshop on Something"), "B");
    assert_eq!(get_venue_tier("Unknown Conference"), "C");
}
