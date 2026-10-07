use qindex::models::{get_venue_field, get_venue_tier, normalize_author_name};

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

#[test]
fn test_get_venue_field() {
    assert_eq!(get_venue_field("SOSP"), "Systems");
    assert_eq!(get_venue_field("OSDI"), "Systems");
    assert_eq!(get_venue_field("SIGMOD"), "Database");
    assert_eq!(get_venue_field("VLDB"), "Database");
    assert_eq!(get_venue_field("STOC"), "Theory");
    assert_eq!(get_venue_field("PLDI"), "Programming Languages");
    assert_eq!(get_venue_field("ASPLOS"), "Architecture");
    assert_eq!(get_venue_field("CCS"), "Security");
    assert_eq!(get_venue_field("Unknown"), "General");
}
