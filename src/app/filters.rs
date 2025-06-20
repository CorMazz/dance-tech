//! Filters for Askama templates

/// Trims off the last `n` characters from the string.
pub fn trim_end_chars<T: std::fmt::Display>(
    s: T,
    _: &dyn askama::Values,
    n: usize,
) -> ::askama::Result<String> {
    let s = s.to_string();
    let trimmed = if s.len() > n {
        s[..s.len() - n].to_string()
    } else {
        s // If `n` is larger than the length of the string, return the string as is
    };
    Ok(trimmed)
}

///  Used to parse the failure explanations, which are stored as strings that are 3 or 4 elements long delimited by .-.-
pub fn split(
    input: &str,
    _: &dyn askama::Values,
    delimiter: &str,
) -> ::askama::Result<Vec<String>> {
    Ok(input.split(delimiter).map(|s| s.to_string()).collect())
}
