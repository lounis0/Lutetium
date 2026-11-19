use crate::event::Key;

/// Append one valid character to a component input. Numeric fields support a
/// partial decimal representation (including `-` and `.` while editing) but
/// reject misplaced signs and repeated decimal points.
pub(super) fn input_char(value: &mut String, code: u32, numeric: bool) -> bool {
    if code == 8 {
        let previous_len = value.len();
        value.pop();
        return value.len() != previous_len;
    }
    let Some(ch) = char::from_u32(code) else {
        return false;
    };
    if ch.is_control() {
        return false;
    }
    if numeric {
        let mut decimal_points = 0;
        for (index, candidate) in value.chars().chain(std::iter::once(ch)).enumerate() {
            if candidate == '-' {
                if index != 0 {
                    return false;
                }
            } else if candidate == '.' {
                decimal_points += 1;
                if decimal_points > 1 {
                    return false;
                }
            } else if !candidate.is_ascii_digit() {
                return false;
            }
        }
    }
    value.push(ch);
    true
}

/// Convert a Windows virtual key to the backend-neutral key type.
pub(super) fn key(value: u32) -> Key {
    match value {
        0x0d => Key::Enter,
        0x20 => Key::Space,
        0x1b => Key::Escape,
        0x09 => Key::Tab,
        0x08 => Key::Backspace,
        0x41..=0x5a => Key::Character(char::from_u32(value).expect("ASCII virtual key")),
        _ => Key::Other(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_input_accepts_partial_decimal_syntax_but_rejects_malformed_values() {
        let mut value = String::new();
        for ch in "-12.5".chars() {
            assert!(input_char(&mut value, ch as u32, true));
        }
        for ch in ['-', '.', 'x', '\n'] {
            assert!(!input_char(&mut value, ch as u32, true));
        }
        assert_eq!(value, "-12.5");
        assert!(input_char(&mut value, 8, true));
        assert_eq!(value, "-12.");
    }

    #[test]
    fn text_input_rejects_control_characters() {
        let mut value = String::new();
        assert!(input_char(&mut value, 'A' as u32, false));
        assert!(!input_char(&mut value, 1, false));
        assert_eq!(value, "A");
    }
}
