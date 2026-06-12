//! # makecss-core
//!
//! CSS 문법으로 네이티브 UI를 그리는 엔진의 '코어(주방)'.
//! 창 띄우기·마우스 같은 바깥세상은 전혀 모릅니다. 오직 일만 합니다:
//!
//! ```text
//!   CSS 텍스트 + 요소 트리  ──→  픽셀(Canvas)
//! ```
//!
//! 내부는 네 단계 파이프라인입니다(각 모듈이 한 단계):
//! ```text
//!   1. css     : 텍스트 → 규칙 구조      (파싱)
//!   2. style   : 규칙 → 요소별 최종 스타일 (캐스케이드)
//!   3. layout  : 스타일 → 위치/크기       (박스 모델)
//!   4. paint   : 박스 → 픽셀              (직접 렌더링)
//! ```

pub mod color;
pub mod css;
pub mod dom;
pub mod font;
pub mod layout;
pub mod paint;
pub mod style;

// 자주 쓰는 타입들을 코어 최상위에서 바로 꺼내 쓸 수 있게 다시 내보냅니다.
pub use color::Color;
pub use dom::Node;
pub use paint::Canvas;

/// 엔진의 정문(고수준 편의 함수).
///
/// 요소 트리 + CSS 텍스트 + 화면 크기를 받아, 완성된 픽셀(Canvas)을 돌려줍니다.
/// 손님은 내부 4단계를 몰라도 이 한 번의 호출로 결과를 얻습니다.
///
/// 비유: 주방에 "이 레시피(css)로 이 재료들(root)을, 이 크기 접시(width×height)에
/// 담아줘" 하고 주문하면, 완성된 요리(Canvas)가 나오는 것.
pub fn render(root: &Node, css: &str, width: u32, height: u32) -> Canvas {
    // 1. 파싱: CSS 텍스트 → 규칙 구조.
    let stylesheet = css::parse(css);

    // 2 + 3. 스타일 계산과 레이아웃: 요소 트리를 화면 폭에 맞춰 배치.
    //        (compute_style은 layout 내부에서 각 요소마다 호출됩니다.)
    let layout_root = layout::layout_tree(root, &stylesheet, width as f32);

    // 4. 페인트: 흰 캔버스를 만들고 박스들을 그 위에 그립니다.
    let mut canvas = Canvas::new(width, height, Color::WHITE);
    paint::paint(&mut canvas, &layout_root);
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_red_box_on_white() {
        // 화면 가득(100x100) 흰 배경 위에, 왼쪽 위에 50x50 빨강 박스.
        let root = Node::new("div").child(Node::new("div").class("box"));
        let css = ".box { width: 50px; height: 50px; background: red; }";
        let canvas = render(&root, css, 100, 100);

        // 박스 안쪽 픽셀(10,10)은 빨강, 박스 밖 픽셀(80,80)은 흰색이어야 합니다.
        let inside = canvas.pixels[10 * 100 + 10];
        let outside = canvas.pixels[80 * 100 + 80];
        assert_eq!(inside, 0xff0000); // 빨강
        assert_eq!(outside, 0xffffff); // 흰색
    }

    #[test]
    fn draws_text_pixels() {
        // 흰 배경에 검은 글자 "I"를 그리면, 글자 픽셀(검정)이 적어도 하나는 생겨야 합니다.
        let root = Node::new("div").class("t").text("I");
        let css = ".t { color: black; font-size: 14px; }";
        let canvas = render(&root, css, 60, 30);
        let has_black = canvas.pixels.iter().any(|&p| p == 0x000000);
        assert!(has_black, "글자 픽셀이 하나도 그려지지 않았습니다");
    }
}
