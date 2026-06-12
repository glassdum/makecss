//! 스타일 매칭과 캐스케이드(cascade).
//!
//! 파서가 만든 규칙(Rule)들을 실제 요소(Node)에 입혀, 그 요소의 '최종 모습'인
//! ComputedStyle(계산된 스타일)을 만듭니다.
//!
//! 한 요소에 여러 규칙이 동시에 맞을 수 있습니다. 충돌하면 누가 이길까요?
//! → 더 '구체적인(specific)' 셀렉터가 이깁니다. 이것이 CSS의 핵심인
//!   캐스케이드와 명시도(specificity)입니다.
//!
//! 명시도 점수(MVP 규칙):
//! ```text
//!   *      (전체)   = 0
//!   div    (태그)   = 1
//!   .card  (클래스) = 10
//! ```
//! 점수가 같으면 '나중에 쓰인 규칙'이 이깁니다(CSS의 실제 규칙과 동일).

use crate::color::Color;
use crate::css::{Declaration, Selector, Stylesheet};
use crate::dom::Node;

/// 한 요소에 대해 모든 충돌을 해결한 '최종' 스타일.
/// 레이아웃과 페인트는 오직 이 구조만 보고 일합니다.
#[derive(Debug, Clone)]
pub struct ComputedStyle {
    /// 내용 영역의 너비(px). None이면 "부모만큼 채움".
    pub width: Option<f32>,
    /// 내용 영역의 높이(px). None이면 "자식들 높이의 합만큼".
    pub height: Option<f32>,
    /// 안쪽 여백(테두리와 내용 사이). MVP는 네 변 같은 값.
    pub padding: f32,
    /// 바깥 여백(이 박스와 옆 박스 사이). MVP는 네 변 같은 값.
    pub margin: f32,
    /// 테두리 두께.
    pub border_width: f32,
    /// 테두리 색.
    pub border_color: Color,
    /// 배경색. 기본은 투명(아무것도 안 칠함).
    pub background: Color,
    /// 글자색.
    pub color: Color,
    /// 글자 크기(px). 비트맵 폰트를 이 크기에 맞춰 확대해 그립니다.
    pub font_size: f32,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        ComputedStyle {
            width: None,
            height: None,
            padding: 0.0,
            margin: 0.0,
            border_width: 0.0,
            border_color: Color::BLACK,
            background: Color::TRANSPARENT,
            color: Color::BLACK,
            font_size: 16.0,
        }
    }
}

/// 한 요소가 주어진 셀렉터에 해당되는지 검사합니다.
/// 비유: 사람의 이름표(tag)와 배지(class)를 보고 "이 규칙은 너에게 해당돼"라고 판정.
fn matches(node: &Node, selector: &Selector) -> bool {
    match selector {
        Selector::Universal => true,
        Selector::Type(name) => &node.tag == name,
        Selector::Class(name) => node.classes.iter().any(|c| c == name),
    }
}

/// 셀렉터의 명시도(구체성) 점수.
fn specificity(selector: &Selector) -> u32 {
    match selector {
        Selector::Universal => 0,
        Selector::Type(_) => 1,
        Selector::Class(_) => 10,
    }
}

/// 한 요소에 적용될 모든 선언을, 이긴 순서대로 모아 ComputedStyle을 만듭니다.
///
/// 동작:
///   1) 스타일시트의 모든 규칙을 훑으며, 이 요소에 맞는 규칙을 찾습니다.
///   2) (명시도, 등장순서) 점수로 정렬합니다 — 약한 것부터 강한 것 순으로.
///   3) 약한 것부터 차례로 덮어씁니다. 그러면 가장 강한 규칙이 최종 값으로 남습니다.
///      (페인트 가게에서 같은 벽에 여러 번 칠하면 마지막 색이 남는 것과 같습니다.)
pub fn compute_style(node: &Node, stylesheet: &Stylesheet) -> ComputedStyle {
    // (점수, 등장순서, 선언들) 묶음을 모읍니다.
    let mut matched: Vec<(u32, usize, &Vec<Declaration>)> = Vec::new();

    for (order, rule) in stylesheet.rules.iter().enumerate() {
        // 한 규칙에 셀렉터가 여러 개면, 그 중 '가장 구체적인' 것으로 점수를 매깁니다.
        let best = rule
            .selectors
            .iter()
            .filter(|sel| matches(node, sel))
            .map(specificity)
            .max();
        if let Some(score) = best {
            matched.push((score, order, &rule.declarations));
        }
    }

    // 약한 것 → 강한 것 순으로 정렬. 점수가 같으면 먼저 쓰인 규칙이 먼저(=나중 것이 이김).
    matched.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));

    // 기본값에서 출발해, 약한 규칙부터 차례로 덮어씁니다.
    let mut style = ComputedStyle::default();
    for (_, _, declarations) in matched {
        for decl in declarations {
            apply_declaration(&mut style, decl);
        }
    }
    style
}

/// 선언 하나("padding: 10px")를 ComputedStyle의 해당 칸에 반영합니다.
/// 알 수 없는 속성이나 해석 실패한 값은 조용히 무시합니다(CSS의 관용적 동작).
fn apply_declaration(style: &mut ComputedStyle, decl: &Declaration) {
    let value = decl.value.trim();
    match decl.property.as_str() {
        "width" => style.width = parse_length(value),
        "height" => style.height = parse_length(value),
        "padding" => {
            if let Some(px) = parse_length(value) {
                style.padding = px;
            }
        }
        "margin" => {
            if let Some(px) = parse_length(value) {
                style.margin = px;
            }
        }
        "border-width" => {
            if let Some(px) = parse_length(value) {
                style.border_width = px;
            }
        }
        "border-color" => {
            if let Some(c) = Color::parse(value) {
                style.border_color = c;
            }
        }
        "background" | "background-color" => {
            if let Some(c) = Color::parse(value) {
                style.background = c;
            }
        }
        "color" => {
            if let Some(c) = Color::parse(value) {
                style.color = c;
            }
        }
        "font-size" => {
            if let Some(px) = parse_length(value) {
                style.font_size = px;
            }
        }
        _ => {} // 모르는 속성은 무시.
    }
}

/// "10px" 또는 "10" 같은 길이 값을 숫자(픽셀)로 바꿉니다.
/// MVP는 px 단위만 지원합니다(%, em 등은 다음 단계).
fn parse_length(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    let number = trimmed.strip_suffix("px").unwrap_or(trimmed);
    number.trim().parse::<f32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css;

    #[test]
    fn class_beats_type() {
        // div 는 파랑, .card 는 빨강. 둘 다 맞지만 .card(클래스)가 더 구체적 → 빨강 승.
        let sheet = css::parse("div { background: blue; } .card { background: red; }");
        let node = Node::new("div").class("card");
        let style = compute_style(&node, &sheet);
        assert_eq!(style.background, Color::rgb(255, 0, 0));
    }

    #[test]
    fn parses_lengths() {
        let sheet = css::parse(".x { width: 200px; padding: 8; }");
        let node = Node::new("div").class("x");
        let style = compute_style(&node, &sheet);
        assert_eq!(style.width, Some(200.0));
        assert_eq!(style.padding, 8.0);
    }
}
