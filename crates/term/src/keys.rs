//! GPUI 键盘事件到 xterm 字节序列的映射。
//!
//! 不依赖 gpui：调用方只传 key / key_char / 修饰键，方便在 CI 做纯逻辑单测。

use alacritty_terminal::term::TermMode;

/// 把一次按键转换成要写给 SSH 通道的字节。
pub fn encode_key(
    key: &str,
    key_char: Option<&str>,
    ctrl: bool,
    alt: bool,
    shift: bool,
    mode: TermMode,
) -> Option<Vec<u8>> {
    let mut bytes = match key {
        "enter" => b"\r".to_vec(),
        "tab" if shift => b"\x1b[Z".to_vec(),
        "tab" => b"\t".to_vec(),
        "backspace" => b"\x7f".to_vec(),
        "escape" => b"\x1b".to_vec(),
        "delete" => b"\x1b[3~".to_vec(),
        "insert" => b"\x1b[2~".to_vec(),
        "home" => b"\x1b[1~".to_vec(),
        "end" => b"\x1b[4~".to_vec(),
        "pageup" => b"\x1b[5~".to_vec(),
        "pagedown" => b"\x1b[6~".to_vec(),
        "up" => arrow('A', mode),
        "down" => arrow('B', mode),
        "right" => arrow('C', mode),
        "left" => arrow('D', mode),
        "f1" => b"\x1bOP".to_vec(),
        "f2" => b"\x1bOQ".to_vec(),
        "f3" => b"\x1bOR".to_vec(),
        "f4" => b"\x1bOS".to_vec(),
        "f5" => b"\x1b[15~".to_vec(),
        "f6" => b"\x1b[17~".to_vec(),
        "f7" => b"\x1b[18~".to_vec(),
        "f8" => b"\x1b[19~".to_vec(),
        "f9" => b"\x1b[20~".to_vec(),
        "f10" => b"\x1b[21~".to_vec(),
        "f11" => b"\x1b[23~".to_vec(),
        "f12" => b"\x1b[24~".to_vec(),
        other => {
            if ctrl {
                control_byte(other)?
            } else {
                let text = key_char.filter(|text| !text.is_empty())?;
                text.as_bytes().to_vec()
            }
        }
    };

    if alt {
        bytes.insert(0, 0x1b);
    }
    Some(bytes)
}

fn arrow(letter: char, mode: TermMode) -> Vec<u8> {
    if mode.contains(TermMode::APP_CURSOR) {
        format!("\x1bO{letter}").into_bytes()
    } else {
        format!("\x1b[{letter}").into_bytes()
    }
}

fn control_byte(key: &str) -> Option<Vec<u8>> {
    let lower = key.to_ascii_lowercase();
    let byte = match lower.as_str() {
        "space" | "2" => 0x00,
        "[" | "3" => 0x1b,
        "\\" | "4" => 0x1c,
        "]" | "5" => 0x1d,
        "^" | "6" => 0x1e,
        "_" | "7" | "/" => 0x1f,
        "?" | "8" => 0x7f,
        other => {
            let mut chars = other.chars();
            let first = chars.next()?;
            if chars.next().is_some() || !first.is_ascii_alphabetic() {
                return None;
            }
            (first.to_ascii_lowercase() as u8) - b'a' + 1
        }
    };
    Some(vec![byte])
}

#[cfg(test)]
mod tests {
    use super::*;

    const NORMAL: TermMode = TermMode::empty();

    #[test]
    fn printable_and_specials() {
        assert_eq!(
            encode_key("a", Some("a"), false, false, false, NORMAL),
            Some(b"a".to_vec())
        );
        assert_eq!(
            encode_key("enter", None, false, false, false, NORMAL),
            Some(b"\r".to_vec())
        );
        assert_eq!(
            encode_key("backspace", None, false, false, false, NORMAL),
            Some(b"\x7f".to_vec())
        );
        assert_eq!(
            encode_key("up", None, false, false, false, NORMAL),
            Some(b"\x1b[A".to_vec())
        );
    }

    #[test]
    fn ctrl_and_alt() {
        assert_eq!(
            encode_key("c", Some("c"), true, false, false, NORMAL),
            Some(vec![0x03])
        );
        assert_eq!(
            encode_key("[", Some("["), true, false, false, NORMAL),
            Some(vec![0x1b])
        );
        assert_eq!(
            encode_key("x", Some("x"), false, true, false, NORMAL),
            Some(b"\x1bx".to_vec())
        );
    }

    #[test]
    fn application_cursor_mode() {
        assert_eq!(
            encode_key(
                "left",
                None,
                false,
                false,
                false,
                TermMode::APP_CURSOR
            ),
            Some(b"\x1bOD".to_vec())
        );
    }

    #[test]
    fn cjk_char_passes_through() {
        assert_eq!(
            encode_key("中", Some("中"), false, false, false, NORMAL),
            Some("中".as_bytes().to_vec())
        );
    }
}
