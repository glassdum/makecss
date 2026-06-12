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
use crate::css::{Declaration, Selector, SimpleSelector, Stylesheet};
use crate::dom::Node;

/// 글자 가로 정렬. text-align 속성의 값.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// 배치 모드. display 속성의 값.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    /// 자식을 위에서 아래로 쌓음(기본).
    Block,
    /// 자식을 한 줄(가로/세로)로 세우는 flex 컨테이너.
    Flex,
    /// 이 요소와 그 자식을 아예 그리지 않음.
    None,
}

/// 위치 지정 방식. position 속성의 값.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    /// 정상 흐름(기본).
    Static,
    /// 정상 흐름 자리는 유지하되 top/left 등으로 시각적으로만 이동.
    Relative,
    /// 흐름에서 빠져나와 '위치 지정된 조상' 기준으로 띄움.
    Absolute,
    /// 흐름에서 빠져나와 '화면(viewport)' 기준으로 띄움.
    Fixed,
}

/// flex 주축 방향.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    Column,
}

/// 주축(main axis) 정렬. justify-content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justify {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
}

/// 교차축(cross axis) 정렬. align-items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignItems {
    Start,
    Center,
    End,
    /// 교차축을 가득 채우도록 늘림(기본).
    Stretch,
}

/// 한 요소에 대해 모든 충돌을 해결한 '최종' 스타일.
/// 레이아웃과 페인트는 오직 이 구조만 보고 일합니다.
#[derive(Debug, Clone, PartialEq)]
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
    /// 글자 크기(px).
    pub font_size: f32,
    /// 글자 가로 정렬(왼쪽/가운데/오른쪽).
    pub text_align: TextAlign,

    // ── 배치/위치 ──
    /// 배치 모드(block/flex/none).
    pub display: Display,
    /// 위치 지정 방식(static/relative/absolute/fixed).
    pub position: Position,
    /// 위치 지정 시 각 변에서의 거리(px). None이면 지정 안 함.
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,

    // ── flex 컨테이너 속성 ──
    /// 주축 방향(가로/세로).
    pub flex_direction: FlexDirection,
    /// 주축 정렬.
    pub justify_content: Justify,
    /// 교차축 정렬.
    pub align_items: AlignItems,
    /// 자식 사이 간격(px).
    pub gap: f32,

    // ── flex 자식 속성 ──
    /// 남는 주축 공간을 나눠 갖는 비율(0이면 안 늘어남).
    pub flex_grow: f32,
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
            text_align: TextAlign::Left,
            display: Display::Block,
            position: Position::Static,
            top: None,
            right: None,
            bottom: None,
            left: None,
            flex_direction: FlexDirection::Row,
            justify_content: Justify::Start,
            align_items: AlignItems::Stretch,
            gap: 0.0,
            flex_grow: 0.0,
        }
    }
}

/// 한 요소가 주어진 셀렉터에 해당되는지 검사합니다.
/// 비유: 사람의 이름표(tag)와 배지(class)를 보고 "이 규칙은 너에게 해당돼"라고 판정.
fn matches(node: &Node, selector: &Selector, hovering: bool) -> bool {
    // 단순 셀렉터가 맞고, :hover 조건이면 지금 hover 중일 때만 맞습니다.
    let simple_ok = match &selector.simple {
        SimpleSelector::Universal => true,
        SimpleSelector::Type(name) => &node.tag == name,
        SimpleSelector::Class(name) => node.classes.iter().any(|c| c == name),
    };
    simple_ok && (!selector.hover || hovering)
}

/// 셀렉터의 명시도(구체성) 점수. :hover 가상클래스는 클래스만큼(+10) 더해집니다.
fn specificity(selector: &Selector) -> u32 {
    let base = match &selector.simple {
        SimpleSelector::Universal => 0,
        SimpleSelector::Type(_) => 1,
        SimpleSelector::Class(_) => 10,
    };
    base + if selector.hover { 10 } else { 0 }
}

/// 한 요소에 적용될 모든 선언을, 이긴 순서대로 모아 ComputedStyle을 만듭니다.
///
/// - parent: 부모의 계산된 스타일. color/font-size/text-align 같은 '상속' 속성의
///   출발점이 됩니다(부모로부터 유전). 박스 속성(width/padding 등)은 상속되지 않습니다.
/// - hovering: 지금 이 요소에 마우스가 올라가 있는지. true면 `:hover` 규칙도 적용됩니다.
///
/// 동작:
///   1) 상속 속성을 부모 값으로 초기화하고, 나머지는 기본값에서 출발.
///   2) 맞는 규칙들을 (명시도, 등장순서)로 정렬해 약→강 순으로 덮어씁니다.
pub fn compute_style(
    node: &Node,
    stylesheet: &Stylesheet,
    parent: &ComputedStyle,
    hovering: bool,
) -> ComputedStyle {
    // 1) 상속: 부모로부터 물려받는 속성을 출발점으로.
    let mut style = ComputedStyle {
        color: parent.color,
        font_size: parent.font_size,
        text_align: parent.text_align,
        ..ComputedStyle::default()
    };

    // 2) (점수, 등장순서, 선언들) 묶음을 모읍니다.
    let mut matched: Vec<(u32, usize, &Vec<Declaration>)> = Vec::new();
    for (order, rule) in stylesheet.rules.iter().enumerate() {
        let best = rule
            .selectors
            .iter()
            .filter(|sel| matches(node, sel, hovering))
            .map(specificity)
            .max();
        if let Some(score) = best {
            matched.push((score, order, &rule.declarations));
        }
    }

    // 약한 것 → 강한 것 순. 점수가 같으면 나중 규칙이 이깁니다.
    matched.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
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
        "text-align" => {
            style.text_align = match value {
                "center" => TextAlign::Center,
                "right" => TextAlign::Right,
                _ => TextAlign::Left,
            };
        }
        "display" => {
            style.display = match value {
                "flex" => Display::Flex,
                "none" => Display::None,
                _ => Display::Block,
            };
        }
        "position" => {
            style.position = match value {
                "relative" => Position::Relative,
                "absolute" => Position::Absolute,
                "fixed" => Position::Fixed,
                _ => Position::Static,
            };
        }
        "top" => style.top = parse_length(value),
        "right" => style.right = parse_length(value),
        "bottom" => style.bottom = parse_length(value),
        "left" => style.left = parse_length(value),
        "flex-direction" => {
            style.flex_direction = match value {
                "column" => FlexDirection::Column,
                _ => FlexDirection::Row,
            };
        }
        "justify-content" => {
            style.justify_content = match value {
                "center" => Justify::Center,
                "flex-end" | "end" => Justify::End,
                "space-between" => Justify::SpaceBetween,
                "space-around" => Justify::SpaceAround,
                _ => Justify::Start,
            };
        }
        "align-items" => {
            style.align_items = match value {
                "center" => AlignItems::Center,
                "flex-end" | "end" => AlignItems::End,
                "flex-start" | "start" => AlignItems::Start,
                _ => AlignItems::Stretch,
            };
        }
        "gap" => {
            if let Some(px) = parse_length(value) {
                style.gap = px;
            }
        }
        "flex-grow" => {
            if let Ok(g) = value.parse::<f32>() {
                style.flex_grow = g;
            }
        }
        "flex" => {
            // 단축 속성: 첫 토큰을 flex-grow로 해석(예: "flex: 1").
            if let Some(first) = value.split_whitespace().next() {
                if let Ok(g) = first.parse::<f32>() {
                    style.flex_grow = g;
                }
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

    fn root() -> ComputedStyle {
        ComputedStyle::default()
    }

    #[test]
    fn class_beats_type() {
        // div 는 파랑, .card 는 빨강. 둘 다 맞지만 .card(클래스)가 더 구체적 → 빨강 승.
        let sheet = css::parse("div { background: blue; } .card { background: red; }");
        let node = Node::new("div").class("card");
        let style = compute_style(&node, &sheet, &root(), false);
        assert_eq!(style.background, Color::rgb(255, 0, 0));
    }

    #[test]
    fn parses_lengths() {
        let sheet = css::parse(".x { width: 200px; padding: 8; }");
        let node = Node::new("div").class("x");
        let style = compute_style(&node, &sheet, &root(), false);
        assert_eq!(style.width, Some(200.0));
        assert_eq!(style.padding, 8.0);
    }

    #[test]
    fn color_inherits_but_background_does_not() {
        // 부모가 color 파랑. 자식은 지정 없음 → color는 상속(파랑), background는 기본(투명).
        let sheet = css::parse("");
        let parent = ComputedStyle { color: Color::rgb(0, 0, 255), ..ComputedStyle::default() };
        let child = compute_style(&Node::new("span"), &sheet, &parent, false);
        assert_eq!(child.color, Color::rgb(0, 0, 255)); // 상속됨
        assert_eq!(child.background, Color::TRANSPARENT); // 상속 안 됨
    }

    #[test]
    fn hover_rules_apply_only_when_hovering() {
        let sheet = css::parse(".b { background: white; } .b:hover { background: black; }");
        let node = Node::new("div").class("b");
        let normal = compute_style(&node, &sheet, &root(), false);
        let hovered = compute_style(&node, &sheet, &root(), true);
        assert_eq!(normal.background, Color::WHITE);
        assert_eq!(hovered.background, Color::BLACK);
    }
}
