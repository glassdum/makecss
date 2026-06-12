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
//!   3. layout  : 스타일 → 위치/크기       (박스 모델 + 글자 줄나눔)
//!   4. paint   : 박스 → 픽셀              (직접 렌더링)
//! ```
//!
//! 글자는 FontFace 약속을 지키는 폰트로 그립니다. 두 가지 폰트가 있습니다:
//!   - font::BitmapFont : 내장 5x7 비트맵 폰트(파일 불필요, 기본값)
//!   - truetype::TtfFont : 진짜 .ttf 파일에서 외곽선을 읽어 그리는 폰트

pub mod color;
pub mod css;
pub mod dom;
pub mod font;
pub mod fontface;
pub mod html;
pub mod layout;
pub mod paint;
pub mod style;
pub mod text;
pub mod truetype;

// 자주 쓰는 타입들을 코어 최상위에서 바로 꺼내 쓸 수 있게 다시 내보냅니다.
pub use color::Color;
pub use dom::Node;
pub use fontface::FontFace;
pub use paint::Canvas;

/// 엔진의 정문(고수준 편의 함수). 내장 비트맵 폰트로 그립니다.
///
/// 요소 트리 + CSS 텍스트 + 화면 크기를 받아, 완성된 픽셀(Canvas)을 돌려줍니다.
/// 손님은 내부 4단계를 몰라도 이 한 번의 호출로 결과를 얻습니다.
pub fn render(root: &Node, css: &str, width: u32, height: u32) -> Canvas {
    render_with_font(root, css, width, height, &font::BitmapFont)
}

/// 정문(폰트 지정 버전). 비트맵이든 TTF든 원하는 폰트로 그립니다.
///
/// 비유: 같은 주방, 같은 레시피라도 '어떤 도장틀(폰트)'을 쓰느냐만 바꾸는 것.
pub fn render_with_font(
    root: &Node,
    css: &str,
    width: u32,
    height: u32,
    font: &dyn FontFace,
) -> Canvas {
    render_core(root, css, width, height, font, None)
}

/// 모든 render 함수가 거쳐가는 실제 구현. pointer가 있으면 :hover를 반영합니다.
fn render_core(
    root: &Node,
    css: &str,
    width: u32,
    height: u32,
    font: &dyn FontFace,
    pointer: Option<(f32, f32)>,
) -> Canvas {
    // 1. 파싱: CSS 텍스트 → 규칙 구조.
    let stylesheet = css::parse(css);

    // 2 + 3. 스타일 계산과 레이아웃(상속·글자 줄나눔 포함)을 화면 크기에 맞춰 수행.
    let layout_root = layout::layout_tree(root, &stylesheet, width as f32, height as f32, font);

    // 4. 페인트: 흰 캔버스를 만들고 박스들을 그 위에 그립니다(포인터로 hover 반영).
    let mut canvas = Canvas::new(width, height, Color::WHITE);
    paint::paint(&mut canvas, &layout_root, font, pointer);
    canvas
}

/// HTML + CSS 문자열을 받아 바로 그립니다(내장 비트맵 폰트).
///
/// 요소 트리를 Rust 코드로 조립하는 대신 `<div class="card">...</div>` 처럼
/// 마크업으로 작성할 수 있습니다.
pub fn render_html(html: &str, css: &str, width: u32, height: u32) -> Canvas {
    let root = html::parse(html);
    render(&root, css, width, height)
}

/// render_html의 폰트 지정 버전(TTF 등).
pub fn render_html_with_font(
    html: &str,
    css: &str,
    width: u32,
    height: u32,
    font: &dyn FontFace,
) -> Canvas {
    let root = html::parse(html);
    render_with_font(&root, css, width, height, font)
}

/// 마우스 위치를 반영해 그립니다(:hover). 인터랙티브 앱(창)에서 매 프레임 호출합니다.
/// pointer가 None이면 hover 없이 그립니다(마우스가 창 밖).
pub fn render_html_with_font_hover(
    html: &str,
    css: &str,
    width: u32,
    height: u32,
    font: &dyn FontFace,
    pointer: Option<(f32, f32)>,
) -> Canvas {
    let root = html::parse(html);
    render_core(&root, css, width, height, font, pointer)
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
        let has_black = canvas.pixels.contains(&0x000000);
        assert!(has_black, "글자 픽셀이 하나도 그려지지 않았습니다");
    }

    #[test]
    fn child_text_inherits_parent_color() {
        // 부모 .box에만 color 빨강 지정. 자식 글자도 빨강을 물려받아야 함.
        let html = r#"<div class="box"><div class="t">I</div></div>"#;
        let css = ".box { color: red; } .t { font-size: 14px; }";
        let canvas = render_html(html, css, 60, 30);
        assert!(canvas.pixels.contains(&0xff0000), "글자가 상속된 빨강이어야 함");
        assert!(!canvas.pixels.contains(&0x000000), "검정 글자가 있으면 상속 실패");
    }

    #[test]
    fn hover_changes_background_under_pointer() {
        let html = r#"<div class="btn"></div>"#;
        let css = ".btn { width: 40px; height: 40px; background: white; } \
                   .btn:hover { background: red; }";
        // 마우스가 박스 밖(95,5) → 흰색. 박스 안(5,5) → 빨강.
        let out = render_html_with_font_hover(html, css, 100, 50, &font::BitmapFont, Some((95.0, 5.0)));
        let on = render_html_with_font_hover(html, css, 100, 50, &font::BitmapFont, Some((5.0, 5.0)));
        assert_eq!(out.pixels[5 * 100 + 5], 0xffffff);
        assert_eq!(on.pixels[5 * 100 + 5], 0xff0000);
    }
}
