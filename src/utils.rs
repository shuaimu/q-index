use colored::Colorize;

pub fn print_banner() {
    let banner = r#"
╔═══════════════════════════════════════════════════════╗
║                                                       ║
║   ██████╗ ██╗███╗   ██╗██████╗ ███████╗██╗  ██╗     ║
║  ██╔═══██╗██║████╗  ██║██╔══██╗██╔════╝╚██╗██╔╝     ║
║  ██║   ██║██║██╔██╗ ██║██║  ██║█████╗   ╚███╔╝      ║
║  ██║▄▄ ██║██║██║╚██╗██║██║  ██║██╔══╝   ██╔██╗      ║
║  ╚██████╔╝██║██║ ╚████║██████╔╝███████╗██╔╝ ██╗     ║
║   ╚══▀▀═╝ ╚═╝╚═╝  ╚═══╝╚═════╝ ╚══════╝╚═╝  ╚═╝     ║
║                                                       ║
║     Academic Quality Index Calculator v0.1.0         ║
║                                                       ║
╚═══════════════════════════════════════════════════════╝
"#;
    println!("{}", banner.cyan().bold());
}

pub fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let mut count = 0;

    for c in s.chars().rev() {
        if count == 3 {
            result.push(',');
            count = 0;
        }
        result.push(c);
        count += 1;
    }

    result.chars().rev().collect()
}

pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

pub fn progress_bar(current: usize, total: usize, width: usize) -> String {
    let progress = (current as f64 / total as f64 * width as f64) as usize;
    let remaining = width - progress;

    format!(
        "[{}{}] {}/{}",
        "=".repeat(progress).green(),
        " ".repeat(remaining),
        current,
        total
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1000000), "1,000,000");
        assert_eq!(format_number(123), "123");
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
    }
}
