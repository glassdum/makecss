//! 색(Color) 타입과 CSS 색 문자열 파싱.
//!
//! 컴퓨터 화면의 모든 색은 빛의 삼원색을 섞어 만듭니다:
//!   R = 빨강(Red), G = 초록(Green), B = 파랑(Blue), 각각 0~255.
//! 여기에 A = 알파(Alpha, 불투명도)를 더합니다. 0 = 완전 투명, 255 = 완전 불투명.
//!
//! 비유: 물감 4통(빨강/초록/파랑/투명도)을 섞어 원하는 색을 만든다고 생각하세요.

/// 하나의 색. 각 채널은 0~255의 정수.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    /// 완전히 투명한 색. "배경색 없음"의 기본값으로 씁니다.
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    /// CSS 색 문자열을 Color로 변환합니다.
    /// 지원: "#rgb", "#rrggbb", "rgb(r,g,b)", "rgba(r,g,b,a)", 그리고 몇 가지 이름.
    /// 비유: "빨강"이라는 '말'을 실제 '물감 배합표'로 번역하는 사전.
    pub fn parse(input: &str) -> Option<Color> {
        let s = input.trim();

        if let Some(hex) = s.strip_prefix('#') {
            return parse_hex(hex);
        }
        if let Some(inner) = s.strip_prefix("rgba(").and_then(|x| x.strip_suffix(')')) {
            return parse_rgb_func(inner, true);
        }
        if let Some(inner) = s.strip_prefix("rgb(").and_then(|x| x.strip_suffix(')')) {
            return parse_rgb_func(inner, false);
        }

        // 자주 쓰는 색 이름 몇 가지. 나중에 얼마든지 늘릴 수 있습니다.
        let named = match s.to_ascii_lowercase().as_str() {
            "black" => Color::BLACK,
            "white" => Color::WHITE,
            "red" => Color::rgb(255, 0, 0),
            "green" => Color::rgb(0, 128, 0),
            "blue" => Color::rgb(0, 0, 255),
            "gray" | "grey" => Color::rgb(128, 128, 128),
            "lightgray" | "lightgrey" => Color::rgb(211, 211, 211),
            "orange" => Color::rgb(255, 165, 0),
            "transparent" => Color::TRANSPARENT,
            _ => return None,
        };
        Some(named)
    }
}

/// "#fff" 또는 "#ffffff" 같은 16진수 색을 해석합니다.
fn parse_hex(hex: &str) -> Option<Color> {
    match hex.len() {
        // #rgb 축약형: 각 글자를 두 번 반복 (예: f → ff).
        3 => {
            let r = hex_digit(hex, 0)? * 17;
            let g = hex_digit(hex, 1)? * 17;
            let b = hex_digit(hex, 2)? * 17;
            Some(Color::rgb(r, g, b))
        }
        // #rrggbb 정식형.
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color::rgb(r, g, b))
        }
        _ => None,
    }
}

/// 16진수 한 글자(0~f)를 0~15 숫자로.
fn hex_digit(s: &str, i: usize) -> Option<u8> {
    s.get(i..i + 1)
        .and_then(|c| u8::from_str_radix(c, 16).ok())
}

/// "255, 0, 0" 또는 "255, 0, 0, 0.5" 같은 rgb()/rgba() 내부를 해석합니다.
fn parse_rgb_func(inner: &str, has_alpha: bool) -> Option<Color> {
    let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
    let expected = if has_alpha { 4 } else { 3 };
    if parts.len() != expected {
        return None;
    }
    let r = parts[0].parse::<u8>().ok()?;
    let g = parts[1].parse::<u8>().ok()?;
    let b = parts[2].parse::<u8>().ok()?;
    let a = if has_alpha {
        // 알파는 0.0~1.0 실수로 들어오므로 0~255로 변환.
        let af = parts[3].parse::<f32>().ok()?;
        (af.clamp(0.0, 1.0) * 255.0).round() as u8
    } else {
        255
    };
    Some(Color::rgba(r, g, b, a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_and_names() {
        assert_eq!(Color::parse("#fff"), Some(Color::WHITE));
        assert_eq!(Color::parse("#ff0000"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(Color::parse("red"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(Color::parse("rgb(0, 128, 0)"), Some(Color::rgb(0, 128, 0)));
        assert_eq!(Color::parse("rgba(0,0,0,0)"), Some(Color::TRANSPARENT));
        assert_eq!(Color::parse("nope"), None);
    }
}
