//! 레이아웃: 각 요소의 위치(x, y)와 크기(width, height)를 계산합니다.
//!
//! ── CSS 박스 모델 ──
//! 모든 요소는 '액자에 든 그림'입니다. 안에서 바깥 순서로 네 겹:
//!
//! ```text
//!   margin (바깥 여백)   ← 옆 액자와의 간격
//!   ┌───────────────────┐
//!   │ border (테두리)    │
//!   │  ┌──────────────┐  │
//!   │  │ padding(안쪽)│  │ ← 내용과 테두리 사이의 빈 공간
//!   │  │  ┌────────┐  │  │
//!   │  │  │content │  │  │ ← 실제 내용(글자/자식 박스)이 들어가는 영역
//!   │  │  └────────┘  │  │
//!   │  └──────────────┘  │
//!   └───────────────────┘
//! ```
//!
//! MVP의 배치 규칙: '블록 흐름(block flow)'. 자식들을 위에서 아래로 차곡차곡 쌓습니다.
//!
//! 계산의 방향이 둘로 나뉘는 점이 중요합니다:
//!   - 너비: 위 → 아래 (부모가 "너는 이만큼 써"라고 자식에게 폭을 내려줌)
//!   - 높이: 아래 → 위 (자식들 높이가 정해져야 그 합으로 부모 높이가 정해짐)

use crate::css::Stylesheet;
use crate::dom::Node;
use crate::font;
use crate::style::{compute_style, ComputedStyle};

/// 화면 위 사각형 영역. 왼쪽 위 모서리 (x, y)와 너비/높이.
/// 단위는 픽셀. 좌표계는 화면 기준: 오른쪽으로 +x, 아래로 +y.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// 레이아웃이 끝난 박스 하나. 페인트 요리사는 오직 이걸 보고 그립니다.
#[derive(Debug, Clone)]
pub struct LayoutBox {
    /// 이 박스의 확정된 스타일(색, 테두리 등).
    pub style: ComputedStyle,
    /// 테두리 바깥선 기준 사각형(= border-box). 배경/테두리를 칠할 영역.
    pub border_box: Rect,
    /// 이 박스가 직접 그릴 글자 내용(있으면). 페인트 단계에서 사용합니다.
    pub text: Option<String>,
    /// 레이아웃이 끝난 자식 박스들.
    pub children: Vec<LayoutBox>,
}

/// 문서 트리 + 스타일시트 + 화면 너비를 받아, 레이아웃된 박스 트리를 만듭니다.
/// 비유: 액자들을 실제 벽(viewport_width 폭의 벽)에 거는 작업.
pub fn layout_tree(root: &Node, stylesheet: &Stylesheet, viewport_width: f32) -> LayoutBox {
    // 최상위 요소는 화면 왼쪽 위(0, 0)에서 시작하고, 가용 폭은 화면 전체 폭.
    layout_node(root, stylesheet, 0.0, 0.0, viewport_width)
}

/// 요소 하나를 (origin_x, origin_y) 위치에, available_width 폭 안에서 배치합니다.
/// available_width = 이 요소가 바깥 여백/테두리/안쪽 여백까지 모두 써도 되는 가로 공간.
fn layout_node(
    node: &Node,
    stylesheet: &Stylesheet,
    origin_x: f32,
    origin_y: f32,
    available_width: f32,
) -> LayoutBox {
    let style = compute_style(node, stylesheet);

    // 한 변에서 '내용 바깥쪽'이 차지하는 두께 = margin + border + padding.
    let edge = style.margin + style.border_width + style.padding;

    // ── 1) 너비 결정 (위 → 아래) ──
    // width가 지정됐으면 그대로 내용 너비로. 아니면 가용 폭에서 양옆 여백을 빼고 채웁니다.
    let content_width = match style.width {
        Some(w) => w,
        None => (available_width - 2.0 * edge).max(0.0),
    };

    // border-box(테두리 기준 사각형)의 왼쪽 위 = 바깥 여백만큼 안으로 들어간 지점.
    let border_x = origin_x + style.margin;
    let border_y = origin_y + style.margin;
    // border-box 너비 = 내용 너비 + 양쪽(테두리+안쪽여백).
    let border_box_width = content_width + 2.0 * (style.border_width + style.padding);

    // 내용 영역의 왼쪽 위 = border-box에서 테두리+안쪽여백만큼 더 들어간 지점.
    let content_x = border_x + style.border_width + style.padding;
    let content_y = border_y + style.border_width + style.padding;

    // ── 2) 자식들을 위에서 아래로 쌓기 (그러면서 높이를 아래→위로 모음) ──
    let mut children = Vec::new();
    let mut cursor_y = content_y; // 다음 자식을 놓을 세로 위치.
    for child in &node.children {
        let child_box = layout_node(child, stylesheet, content_x, cursor_y, content_width);
        // 다음 자식은 이 자식의 '바깥 여백 포함 전체 높이'만큼 아래로.
        cursor_y += outer_height(&child_box);
        children.push(child_box);
    }

    // 자식들이 실제로 차지한 내용 높이 = 마지막 커서 - 내용 시작점.
    let children_height = cursor_y - content_y;

    // ── 3) 높이 결정 ──
    // height가 지정됐으면 그 값을, 아니면 '내용이 스스로 차지하는 높이'를 씁니다.
    // - 자식이 있으면: 자식들이 쌓인 높이.
    // - 자식 없이 글자만 있으면: 글자 한 줄 높이.
    let intrinsic_height = if node.children.is_empty() && node.text.is_some() {
        font::line_height(style.font_size)
    } else {
        children_height
    };
    let content_height = style.height.unwrap_or(intrinsic_height);
    let border_box_height = content_height + 2.0 * (style.border_width + style.padding);

    LayoutBox {
        style,
        border_box: Rect {
            x: border_x,
            y: border_y,
            width: border_box_width,
            height: border_box_height,
        },
        text: node.text.clone(),
        children,
    }
}

/// 박스가 세로로 실제 차지하는 전체 높이 = 위아래 바깥 여백 + 테두리박스 높이.
fn outer_height(b: &LayoutBox) -> f32 {
    b.style.margin + b.border_box.height + b.style.margin
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css;

    #[test]
    fn fills_parent_width_minus_padding() {
        // 부모 폭 300, padding 10 → 내용 폭은 300 - 20 = 280. border-box는 300.
        let sheet = css::parse(".box { padding: 10px; }");
        let node = Node::new("div").class("box");
        let root = layout_tree(&node, &sheet, 300.0);
        assert_eq!(root.border_box.width, 300.0);
    }

    #[test]
    fn height_grows_from_children() {
        // 자식 둘이 각각 높이 50 → 부모 높이는 100(여기선 여백/테두리 없음).
        let sheet = css::parse(".child { height: 50px; }");
        let node = Node::new("div")
            .child(Node::new("div").class("child"))
            .child(Node::new("div").class("child"));
        let root = layout_tree(&node, &sheet, 200.0);
        assert_eq!(root.border_box.height, 100.0);
    }
}
