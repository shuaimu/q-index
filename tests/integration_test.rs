use qindex::models::normalize_author_name;

#[test]
fn test_normalize_author_name() {
    assert_eq!(normalize_author_name("John Doe"), "John Doe");
    assert_eq!(normalize_author_name("Doe, John"), "John Doe");
    assert_eq!(normalize_author_name("john doe"), "John Doe");
    assert_eq!(normalize_author_name("J. Doe"), "J Doe");
    assert_eq!(normalize_author_name("  John   Doe  "), "John Doe");
}
