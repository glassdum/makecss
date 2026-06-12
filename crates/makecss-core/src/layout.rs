//! 레이아웃: 각 요소의 위치(x, y)와 크기(width, height)를 계산합니다.
//!
//! ── CSS 박스 모델 ──
//! 모든 요소는 '액자에 든 그림'입니다. 안에서 바깥 순서로 네 겹:
//!
//! ```text
//!   margin (바깥 여백)
//!   ┌───────────────────┐
//!   │ border (테두리)    │
//!   │  ┌──────────────┐  │
//!   │  │ padding(안쪽)│  │
//!   │  │  ┌────────┐  │  │
//!   │  │  │content │  │  │ ← 글자/자식 박스가 들어가는 영역
//!   │  │  └────────┘  │  │
//!   │  └──────────────┘  │
//!   └───────────────────┘
//! ```
//!
//! ── 배치 모드(display) ──
//!   - block : 자식을 위에서 아래로 쌓음(기본).
//!   - flex  : 자식을 한 줄(가로 row / 세로 column)로 세우고 남는 공간을 나눔.
//!   - none  : 아예 그리지 않음.
//!
//! ── 위치 지정(position) ──
//!   - static   : 정상 흐름(기본).
//!   - relative : 자리는 유지하되 top/left 등으로 시각적으로만 이동.
//!   - absolute : 흐름에서 빠져나와 '위치 지정된 부모' 기준으로 띄움(배지/오버레이).
//!   - fixed    : 흐름에서 빠져나와 '화면' 기준으로 띄움.
//!
//! 구현 메모: flex 자식과 흐름 밖(absolute/fixed) 요소는 '먼저 (0,0) 부근에 배치한 뒤
//! 최종 위치로 통째로 옮기는(translate)' 방식으로 처리합니다. 그러면 그 안의 절대
//! 위치 자식도 함께 움직여 좌표가 일관됩니다.

use crate::css::Stylesheet;
use crate::dom::Node;
use crate::fontface::FontFace;
use crate::style::{
    compute_style, AlignItems, ComputedStyle, Display, FlexDirection, Justify, Position,
};
use crate::text::{self, TextBlock};

/// 화면 위 사각형 영역. 왼쪽 위 모서리 (x, y)와 너비/높이(px).
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// 레이아웃이 끝난 박스 하나. 페인트는 오직 이것만 보고 그립니다.
#[derive(Debug, Clone)]
pub struct LayoutBox {
    pub style: ComputedStyle,
    /// 테두리 바깥선 기준 사각형(= border-box). 배경/테두리를 칠할 영역.
    pub border_box: Rect,
    /// 줄나눔·정렬까지 끝난 글자 내용(있으면).
    pub text: Option<TextBlock>,
    /// 레이아웃이 끝난 자식 박스들.
    pub children: Vec<LayoutBox>,
}

/// 문서 트리 + 스타일시트 + 화면 크기 + 폰트를 받아, 레이아웃된 박스 트리를 만듭니다.
pub fn layout_tree(
    root: &Node,
    stylesheet: &Stylesheet,
    viewport_width: f32,
    viewport_height: f32,
    font: &dyn FontFace,
) -> LayoutBox {
    let viewport = Rect { x: 0.0, y: 0.0, width: viewport_width, height: viewport_height };
    layout_node(root, stylesheet, 0.0, 0.0, viewport_width, viewport, viewport, font).unwrap_or(
        LayoutBox {
            style: ComputedStyle::default(),
            border_box: Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 },
            text: None,
            children: Vec::new(),
        },
    )
}

/// 요소 하나를 배치합니다. display:none이면 None.
///
/// - (origin_x, origin_y): 정상 흐름일 때 이 요소의 '마진 박스'가 시작할 위치.
/// - available_width: 이 요소가 써도 되는 가로 공간(마진/테두리/패딩 포함).
/// - cb: 위치 지정된 가장 가까운 조상의 내용 영역(absolute 기준).
/// - viewport: 화면 전체 영역(fixed 기준).
fn layout_node(
    node: &Node,
    sheet: &Stylesheet,
    origin_x: f32,
    origin_y: f32,
    available_width: f32,
    cb: Rect,
    viewport: Rect,
    font: &dyn FontFace,
) -> Option<LayoutBox> {
    let style = compute_style(node, sheet);
    if style.display == Display::None {
        return None;
    }

    let out_of_flow = matches!(style.position, Position::Absolute | Position::Fixed);
    let establishes_cb = style.position != Position::Static;
    // 흐름 밖 요소의 위치 기준(base): fixed=화면, absolute=조상의 내용 영역.
    let base = match style.position {
        Position::Fixed => viewport,
        Position::Absolute => cb,
        _ => Rect { x: origin_x, y: origin_y, width: available_width, height: viewport.height },
    };

    let (m, bd, pd) = (style.margin, style.border_width, style.padding);
    let edge = m + bd + pd;

    // ── 너비 결정 ──
    let width_basis = if out_of_flow { base.width } else { available_width };
    let content_width = match style.width {
        Some(w) => w,
        None => {
            if out_of_flow {
                if style.left.is_some() && style.right.is_some() {
                    // 좌우가 둘 다 고정되면 그 사이를 채웁니다.
                    (base.width - style.left.unwrap() - style.right.unwrap() - 2.0 * (bd + pd))
                        .max(0.0)
                } else {
                    // 그 외 흐름 밖 요소는 '내용 크기에 맞춰 줄어듭니다'(shrink-to-fit).
                    let stf = (intrinsic_width(node, sheet, font) - 2.0 * edge).max(0.0);
                    stf.min((base.width - 2.0 * edge).max(0.0))
                }
            } else {
                (width_basis - 2.0 * edge).max(0.0)
            }
        }
    };

    // ── '임시' border-box 위치(흐름 밖이면 base 기준; 나중에 offset만큼 translate) ──
    let (eff_x, eff_y) = if out_of_flow { (base.x, base.y) } else { (origin_x, origin_y) };
    let bx = eff_x + m;
    let by = eff_y + m;
    let cx = bx + bd + pd; // 내용 영역 왼쪽 위.
    let cy = by + bd + pd;
    let border_box_width = content_width + 2.0 * (bd + pd);

    // 자식들에게 넘겨줄 containing block(이 요소가 위치 지정됐다면 자기 내용 영역).
    let cb_children = if establishes_cb {
        Rect { x: cx, y: cy, width: content_width, height: style.height.unwrap_or(0.0) }
    } else {
        cb
    };

    // ── 자식들을 흐름 안/밖으로 나눕니다(none은 버림) ──
    let mut in_flow: Vec<&Node> = Vec::new();
    let mut out_flow: Vec<&Node> = Vec::new();
    for child in &node.children {
        let cs = compute_style(child, sheet);
        if cs.display == Display::None {
            continue;
        }
        if matches!(cs.position, Position::Absolute | Position::Fixed) {
            out_flow.push(child);
        } else {
            in_flow.push(child);
        }
    }

    // ── 글자(있으면)를 내용 폭에 맞춰 줄나눔 ──
    let text_block = node
        .text
        .as_ref()
        .map(|t| text::layout_text(t, font, style.font_size, content_width, style.text_align));

    // ── 흐름 안 자식 배치(block 또는 flex) ──
    let (mut children, children_height) = if style.display == Display::Flex {
        layout_flex(&style, cx, cy, content_width, &in_flow, sheet, cb_children, viewport, font)
    } else {
        layout_block(cx, cy, content_width, &in_flow, sheet, cb_children, viewport, font)
    };

    // ── 높이 결정 ──
    let intrinsic_height = match &text_block {
        Some(tb) => tb.height,
        None => children_height,
    };
    let content_height = style.height.unwrap_or(intrinsic_height);
    let border_box_height = content_height + 2.0 * (bd + pd);

    // ── 흐름 밖 자식들은 '확정된 내용 영역'을 기준으로 배치 ──
    let cb_final = if establishes_cb {
        Rect { x: cx, y: cy, width: content_width, height: content_height }
    } else {
        cb
    };
    for child in out_flow {
        if let Some(b) = layout_node(child, sheet, cx, cy, content_width, cb_final, viewport, font) {
            children.push(b);
        }
    }

    let mut layout_box = LayoutBox {
        style: style.clone(),
        border_box: Rect { x: bx, y: by, width: border_box_width, height: border_box_height },
        text: text_block,
        children,
    };

    // ── 위치 보정 ──
    if out_of_flow {
        // 임시로 base 위치에 두었으니, top/left(또는 right/bottom)만큼 옮깁니다.
        let outer_w = border_box_width + 2.0 * m;
        let outer_h = border_box_height + 2.0 * m;
        let off_x = match (style.left, style.right) {
            (Some(l), _) => l,
            (None, Some(r)) => base.width - outer_w - r,
            (None, None) => 0.0,
        };
        let off_y = match (style.top, style.bottom) {
            (Some(t), _) => t,
            (None, Some(b)) => base.height - outer_h - b,
            (None, None) => 0.0,
        };
        translate(&mut layout_box, off_x, off_y);
    } else if style.position == Position::Relative {
        // 자리는 유지하되 시각적으로만 이동.
        let dx = style.left.unwrap_or_else(|| style.right.map(|r| -r).unwrap_or(0.0));
        let dy = style.top.unwrap_or_else(|| style.bottom.map(|b| -b).unwrap_or(0.0));
        translate(&mut layout_box, dx, dy);
    }

    Some(layout_box)
}

/// block 배치: 흐름 안 자식들을 위에서 아래로 쌓습니다.
fn layout_block(
    cx: f32,
    cy: f32,
    content_width: f32,
    children: &[&Node],
    sheet: &Stylesheet,
    cb: Rect,
    viewport: Rect,
    font: &dyn FontFace,
) -> (Vec<LayoutBox>, f32) {
    let mut boxes = Vec::new();
    let mut cursor_y = cy;
    for child in children {
        if let Some(b) = layout_node(child, sheet, cx, cursor_y, content_width, cb, viewport, font) {
            cursor_y += outer_height(&b);
            boxes.push(b);
        }
    }
    (boxes, cursor_y - cy)
}

/// flex 배치: 흐름 안 자식들을 한 줄(가로/세로)로 세웁니다.
/// 절차: ① 자연 크기로 우선 배치 → ② 남는 공간을 flex-grow로 분배 →
///       ③ 교차축 정렬/늘리기 → ④ 주축 정렬(justify) → ⑤ 최종 위치로 이동.
fn layout_flex(
    style: &ComputedStyle,
    cx: f32,
    cy: f32,
    content_width: f32,
    children: &[&Node],
    sheet: &Stylesheet,
    cb: Rect,
    viewport: Rect,
    font: &dyn FontFace,
) -> (Vec<LayoutBox>, f32) {
    let row = style.flex_direction == FlexDirection::Row;
    let gap = style.gap;
    let n = children.len();
    if n == 0 {
        return (Vec::new(), style.height.unwrap_or(0.0));
    }

    // ① 각 자식을 '자연 크기'로 우선 배치하고 주축 크기를 잰다.
    let mut boxes: Vec<LayoutBox> = Vec::with_capacity(n);
    let mut grows: Vec<f32> = Vec::with_capacity(n);
    let mut base_main: Vec<f32> = Vec::with_capacity(n);
    for child in children {
        let cs = compute_style(child, sheet);
        grows.push(cs.flex_grow);
        let avail = if row {
            intrinsic_width(child, sheet, font)
        } else if style.align_items == AlignItems::Stretch {
            content_width
        } else {
            intrinsic_width(child, sheet, font).min(content_width)
        };
        let b = layout_node(child, sheet, 0.0, 0.0, avail, cb, viewport, font)
            .expect("flex 자식은 display:none이 아님");
        base_main.push(if row { outer_width(&b) } else { outer_height(&b) });
        boxes.push(b);
    }

    // ② 남는 주축 공간을 flex-grow 비율로 나눈다.
    //    grow가 있는 자식은 'flex-basis 0'처럼 0에서 시작해 공간을 나눠 갖습니다
    //    (그래서 `flex: 1` 인 카드 셋이 줄을 똑같이 삼등분합니다).
    let total_base: f32 = base_main.iter().sum();
    let total_gap = gap * (n as f32 - 1.0);
    let main_bounded = row || style.height.is_some();
    let main_container = if row {
        content_width
    } else {
        style.height.unwrap_or(total_base + total_gap)
    };
    let total_grow: f32 = grows.iter().sum();
    let mut final_main = base_main.clone();
    if main_bounded && total_grow > 0.0 {
        // grow 없는 자식들이 차지하는 크기를 빼고 남은 공간을 grow 자식들이 나눕니다.
        let nongrow_base: f32 = (0..n).filter(|&i| grows[i] <= 0.0).map(|i| base_main[i]).sum();
        let free = (main_container - nongrow_base - total_gap).max(0.0);
        for i in 0..n {
            if grows[i] > 0.0 {
                final_main[i] = free * grows[i] / total_grow;
            }
        }
    }

    // ③ 주축 크기가 바뀐 자식은 그 크기에 맞춰 반영한다.
    for i in 0..n {
        if (final_main[i] - base_main[i]).abs() > 0.01 {
            if row {
                // 가로: 너비가 바뀌면 줄나눔이 달라질 수 있어 다시 배치.
                boxes[i] = layout_node(children[i], sheet, 0.0, 0.0, final_main[i], cb, viewport, font)
                    .unwrap();
            } else {
                // 세로: 높이만 늘린다.
                let mm = boxes[i].style.margin;
                boxes[i].border_box.height = (final_main[i] - 2.0 * mm).max(0.0);
            }
        }
    }

    // ④ 교차축 컨테이너 크기.
    let cross_container = if row {
        style.height.unwrap_or_else(|| boxes.iter().map(outer_height).fold(0.0, f32::max))
    } else {
        content_width
    };

    // 교차축 stretch면 각 자식을 가득 채우도록 늘린다.
    if style.align_items == AlignItems::Stretch {
        for b in &mut boxes {
            let mm = b.style.margin;
            if row {
                b.border_box.height = (cross_container - 2.0 * mm).max(0.0);
            } else {
                b.border_box.width = (cross_container - 2.0 * mm).max(0.0);
            }
        }
    }

    // ⑤ 주축 정렬(justify-content)로 시작 위치/간격을 정하고 각 자식을 옮긴다.
    let used_main: f32 = final_main.iter().sum::<f32>() + total_gap;
    let free_main = (main_container - used_main).max(0.0);
    let (start_off, between) = justify_offsets(style.justify_content, free_main, n, gap);

    let mut cursor = start_off;
    for i in 0..n {
        let b = &mut boxes[i];
        let item_cross = if row { outer_height(b) } else { outer_width(b) };
        let cross_off = cross_offset(style.align_items, cross_container, item_cross);
        // 목표 '마진 박스' 왼쪽 위.
        let (mx, my) = if row { (cx + cursor, cy + cross_off) } else { (cx + cross_off, cy + cursor) };
        let cur_mx = b.border_box.x - b.style.margin;
        let cur_my = b.border_box.y - b.style.margin;
        translate(b, mx - cur_mx, my - cur_my);
        cursor += final_main[i] + between;
    }

    let content_height = if row { cross_container } else { style.height.unwrap_or(used_main) };
    (boxes, content_height)
}

/// justify-content: 남는 공간(free)을 어떻게 분배할지 → (시작 오프셋, 항목 사이 간격).
fn justify_offsets(j: Justify, free: f32, n: usize, gap: f32) -> (f32, f32) {
    match j {
        Justify::Start => (0.0, gap),
        Justify::Center => (free / 2.0, gap),
        Justify::End => (free, gap),
        Justify::SpaceBetween => {
            let extra = if n > 1 { free / (n as f32 - 1.0) } else { 0.0 };
            (0.0, gap + extra)
        }
        Justify::SpaceAround => {
            let unit = free / n as f32;
            (unit / 2.0, gap + unit)
        }
    }
}

/// align-items: 교차축에서 항목을 어디에 둘지 → 교차 방향 오프셋.
fn cross_offset(a: AlignItems, container_cross: f32, item_cross: f32) -> f32 {
    match a {
        AlignItems::Start | AlignItems::Stretch => 0.0,
        AlignItems::Center => (container_cross - item_cross) / 2.0,
        AlignItems::End => container_cross - item_cross,
    }
}

/// 요소의 '자연 너비'(max-content): 줄나눔 없이 필요한 가로 폭.
/// flex에서 각 자식의 기본 크기를 정할 때 씁니다.
fn intrinsic_width(node: &Node, sheet: &Stylesheet, font: &dyn FontFace) -> f32 {
    let s = compute_style(node, sheet);
    let ex = 2.0 * (s.margin + s.border_width + s.padding);
    let content = if let Some(w) = s.width {
        w
    } else if let Some(t) = &node.text {
        // 줄나눔 없이 한 줄로 폈을 때의 글자 폭(공백 포함).
        t.chars().map(|c| font.advance(c, s.font_size)).sum()
    } else if node.children.is_empty() {
        0.0
    } else {
        let row = s.display == Display::Flex && s.flex_direction == FlexDirection::Row;
        let kids: Vec<f32> = node
            .children
            .iter()
            .filter(|c| compute_style(c, sheet).display != Display::None)
            .map(|c| intrinsic_width(c, sheet, font))
            .collect();
        if row {
            kids.iter().sum::<f32>() + s.gap * ((kids.len() as f32 - 1.0).max(0.0))
        } else {
            kids.iter().cloned().fold(0.0, f32::max)
        }
    };
    content + ex
}

/// 박스와 그 자식들을 통째로 (dx, dy) 만큼 옮깁니다.
/// (글자 위치는 paint 단계에서 border_box로부터 계산하므로 함께 따라옵니다.)
fn translate(b: &mut LayoutBox, dx: f32, dy: f32) {
    b.border_box.x += dx;
    b.border_box.y += dy;
    for c in &mut b.children {
        translate(c, dx, dy);
    }
}

/// 박스가 가로로 차지하는 전체 너비 = 좌우 바깥 여백 + 테두리박스 너비.
fn outer_width(b: &LayoutBox) -> f32 {
    2.0 * b.style.margin + b.border_box.width
}

/// 박스가 세로로 차지하는 전체 높이 = 위아래 바깥 여백 + 테두리박스 높이.
fn outer_height(b: &LayoutBox) -> f32 {
    2.0 * b.style.margin + b.border_box.height
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css;
    use crate::font::BitmapFont;

    fn layout(html_css: (&str, &str), w: f32) -> LayoutBox {
        let node = crate::html::parse(html_css.0);
        let sheet = css::parse(html_css.1);
        layout_tree(&node, &sheet, w, 600.0, &BitmapFont)
    }

    #[test]
    fn fills_parent_width_minus_padding() {
        let root = layout((r#"<div class="box"></div>"#, ".box { padding: 10px; }"), 300.0);
        assert_eq!(root.border_box.width, 300.0);
    }

    #[test]
    fn block_stacks_children_vertically() {
        let root = layout(
            (
                r#"<div><div class="c"></div><div class="c"></div></div>"#,
                ".c { height: 50px; }",
            ),
            200.0,
        );
        assert_eq!(root.border_box.height, 100.0);
        // 둘째 자식이 첫째 아래에 위치.
        assert!(root.children[1].border_box.y > root.children[0].border_box.y);
    }

    #[test]
    fn flex_row_places_children_side_by_side() {
        let root = layout(
            (
                r#"<div class="row"><div class="a"></div><div class="b"></div></div>"#,
                ".row { display: flex; } .a, .b { width: 40px; height: 20px; }",
            ),
            300.0,
        );
        let a = &root.children[0];
        let b = &root.children[1];
        // 가로 배치: 같은 y, 다른 x.
        assert_eq!(a.border_box.y, b.border_box.y);
        assert!(b.border_box.x > a.border_box.x);
    }

    #[test]
    fn flex_grow_fills_remaining_width() {
        let root = layout(
            (
                r#"<div class="row"><div class="g"></div></div>"#,
                ".row { display: flex; } .g { flex: 1; height: 20px; }",
            ),
            200.0,
        );
        // flex:1 자식이 컨테이너 폭(200)을 가득 채움.
        assert!((root.children[0].border_box.width - 200.0).abs() < 1.0);
    }

    #[test]
    fn display_none_is_skipped() {
        let root = layout(
            (
                r#"<div><div class="gone"></div><div class="shown" ></div></div>"#,
                ".gone { display: none; height: 50px; } .shown { height: 30px; }",
            ),
            100.0,
        );
        // none은 자식 목록에 없음 → 보이는 자식 하나뿐, 높이 30.
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.border_box.height, 30.0);
    }

    #[test]
    fn absolute_positions_relative_to_positioned_parent() {
        let root = layout(
            (
                r#"<div class="card"><div class="badge"></div></div>"#,
                ".card { position: relative; height: 100px; padding: 0px; } \
                 .badge { position: absolute; top: 10px; left: 20px; width: 8px; height: 8px; }",
            ),
            100.0,
        );
        let badge = &root.children[0];
        // 카드 내용 영역(0,0) 기준 top:10 left:20 → 마진 박스 (20,10).
        assert!((badge.border_box.x - 20.0).abs() < 0.5);
        assert!((badge.border_box.y - 10.0).abs() < 0.5);
    }
}
