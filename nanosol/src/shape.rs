//! Deterministic helpers for compact, single-line, injection-resistant output.

use crate::pubkey::Pubkey;

pub fn elide_address(address: &Pubkey) -> String {
    let encoded = address.to_string();
    if encoded.chars().count() <= 10 {
        return encoded;
    }
    let start: String = encoded.chars().take(4).collect();
    let end: String = encoded
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{start}…{end}")
}

/// Normalize untrusted text to one line and truncate by Unicode scalar count.
pub fn single_line(input: &str, max_chars: usize) -> String {
    let normalized: String = input
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let mut output: String = normalized.chars().take(max_chars).collect();
    if normalized.chars().count() > max_chars {
        output.push('…');
    }
    output
}

pub fn quote_untrusted(input: &str, max_chars: usize) -> String {
    let normalized = single_line(input, max_chars).replace('\'', "’");
    format!("'{normalized}'")
}
