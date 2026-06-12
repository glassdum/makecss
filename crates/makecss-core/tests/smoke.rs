//! 스모크 테스트: 다양한 극단 입력에서 '패닉이 나지 않는지'를 확인합니다.
//! (정확한 픽셀값보다, 깨진 입력에도 엔진이 죽지 않는 견고함을 검증)

use makecss_core::render_html;

/// 한 케이스를 렌더합니다. 내부에서 패닉이 나면 테스트가 실패합니다.
fn r(html: &str, css: &str, w: u32, h: u32) {
    let canvas = render_html(html, css, w, h);
    assert_eq!(canvas.pixels.len(), (w * h) as usize);
}

#[test]
fn empty_inputs() {
    r("", "", 50, 50);
    r("", ".x { color: red; }", 50, 50);
    r("<div></div>", "", 50, 50);
}

#[test]
fn zero_and_tiny_sizes() {
    r("<div>hi</div>", "div { padding: 4px; }", 0, 0);
    r("<div>hi</div>", "div { padding: 4px; }", 1, 1);
    r("<div>hi</div>", "div { padding: 4px; }", 2, 100);
}

#[test]
fn negative_and_huge_lengths() {
    r("<div>x</div>", "div { width: -10px; }", 80, 80);
    r("<div>x</div>", "div { padding: 100000px; }", 80, 80);
    r("<div>x</div>", "div { margin: -50px; height: -5px; }", 80, 80);
}

#[test]
fn display_none_everywhere() {
    r("<div style></div>", ".gone { display: none; }", 40, 40);
    r(r#"<div class="gone">hi</div>"#, ".gone { display: none; }", 40, 40);
    r(
        r#"<div class="gone"><div>a</div><div>b</div></div>"#,
        ".gone { display: none; }",
        40,
        40,
    );
}

#[test]
fn flex_edge_cases() {
    // 자식 없는 flex
    r(r#"<div class="f"></div>"#, ".f { display: flex; }", 60, 60);
    // 자식 하나 + space-between (n-1=0 나눗셈 주의)
    r(
        r#"<div class="f"><div>a</div></div>"#,
        ".f { display: flex; justify-content: space-between; }",
        60,
        60,
    );
    // flex column
    r(
        r#"<div class="f"><div>a</div><div>b</div></div>"#,
        ".f { display: flex; flex-direction: column; gap: 8px; }",
        60,
        60,
    );
    // 모든 정렬 조합
    for jc in ["flex-start", "center", "flex-end", "space-between", "space-around"] {
        for ai in ["flex-start", "center", "flex-end", "stretch"] {
            let css = format!(
                ".f {{ display: flex; justify-content: {jc}; align-items: {ai}; gap: 6px; }}"
            );
            r(
                r#"<div class="f"><div>aa</div><div>bbbb</div><div>c</div></div>"#,
                &css,
                120,
                60,
            );
        }
    }
}

#[test]
fn position_variants() {
    let css = ".p { position: relative; height: 60px; } \
               .a { position: absolute; top: 5px; left: 5px; width: 10px; height: 10px; } \
               .b { position: absolute; bottom: 5px; right: 5px; width: 10px; height: 10px; } \
               .fx { position: fixed; bottom: 0px; right: 0px; width: 10px; height: 10px; }";
    r(
        r#"<div class="p"><div class="a"></div><div class="b"></div><div class="fx"></div></div>"#,
        css,
        100,
        100,
    );
}

#[test]
fn long_unbreakable_text_and_wrapping() {
    r(
        "<div>supercalifragilisticexpialidocious_a_very_long_unbreakable_token</div>",
        "div { width: 30px; }",
        60,
        120,
    );
    r(
        "<div>one two three four five six seven eight nine ten</div>",
        "div { width: 40px; }",
        60,
        200,
    );
}

#[test]
fn unusual_characters() {
    // 폰트에 없는 글자(한글/이모지/제어문자)도 죽지 않아야 함.
    r("<div>안녕하세요 🚀 café</div>", "div { font-size: 16px; }", 200, 80);
    r("<div>\t\n  spaced  </div>", "div {}", 100, 40);
}

#[test]
fn malformed_html_and_css() {
    r("<div><span>unclosed", "div { color: red }", 80, 40);
    r("<<>><div>></div", "}{ : ; garbage", 80, 40);
    r("<div class=>no value</div>", "div { : ; width }", 80, 40);
    r("<!-- comment only -->", "/* nothing */", 40, 40);
}

#[test]
fn absurd_font_size_does_not_oom() {
    // 비정상적으로 큰 font-size여도 메모리 폭발 없이 끝나야 합니다(글자는 생략될 수 있음).
    r("<div>big</div>", "div { font-size: 100000px; }", 80, 80);
}

#[test]
fn deeply_nested() {
    let mut html = String::new();
    for _ in 0..200 {
        html.push_str("<div>");
    }
    html.push_str("deep");
    for _ in 0..200 {
        html.push_str("</div>");
    }
    r(&html, "div { padding: 1px; }", 300, 300);
}
