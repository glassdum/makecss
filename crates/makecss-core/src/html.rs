//! HTML 파서: `<div class="card">...</div>` 같은 텍스트를 Node 트리로 바꿉니다.
//!
//! CSS 파서와 같은 기법(글자를 왼쪽부터 훑으며 구조 쌓기)을 쓰지만, 결과물이
//! 다릅니다. CSS 파서는 '규칙표'를, HTML 파서는 '요소들의 트리'를 만듭니다.
//!
//! 비유: 여는 태그 <div>는 "새 사람 등장, 자식 구역 시작", 닫는 태그 </div>는
//! "이 사람의 자식 구역 끝". 태그 중첩이 곧 부모-자식 관계가 됩니다.
//!
//! 지원(MVP):
//!   - 요소: <tag class="a b"> ... </tag>, self-closing <tag/>, void 태그(<br> 등)
//!   - 속성: class(공백으로 나눠 클래스 목록), 그 외 속성은 무시
//!   - 텍스트: 태그 사이의 글자. 공백만 있는 부분은 건너뜀
//!   - 주석 <!-- -->, DOCTYPE <!...>, 기본 엔티티(&amp; &lt; &gt; &quot; &#39;)
//!
//! 한 요소가 '텍스트만' 담으면 그 텍스트를 요소 자체에 붙입니다(그 요소의 CSS가
//! 그대로 적용되도록). 자식 요소가 섞이면 글자 조각은 "#text" 노드로 들어갑니다.

use crate::dom::Node;

/// 텍스트 노드(글자 조각)를 나타내는 가짜 태그 이름.
pub const TEXT_TAG: &str = "#text";

/// 자체적으로 닫히는(닫는 태그가 없는) 'void' 요소들.
const VOID_TAGS: &[&str] = &["br", "img", "hr", "input", "meta", "link", "area", "base"];

/// HTML 문자열을 파싱해 하나의 루트 Node로 돌려줍니다.
/// 최상위 요소가 여럿이면 익명 <div>로 감싸고, 하나면 그것을 그대로 돌려줍니다.
pub fn parse(input: &str) -> Node {
    let mut nodes = parse_fragment(input);
    if nodes.len() == 1 {
        nodes.pop().unwrap()
    } else {
        let mut root = Node::new("div");
        root.children = nodes;
        root
    }
}

/// HTML을 파싱해 최상위 노드들의 목록으로 돌려줍니다.
pub fn parse_fragment(input: &str) -> Vec<Node> {
    let mut parser = Parser {
        chars: input.chars().collect(),
        pos: 0,
    };
    parser.parse_nodes()
}

/// 파서 상태: 글자 배열 + 커서 위치(css.rs의 파서와 같은 구조).
struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// 커서가 가리키는 위치가 주어진 문자열로 시작하는가?
    fn starts_with(&self, s: &str) -> bool {
        self.chars[self.pos..]
            .iter()
            .zip(s.chars())
            .filter(|(a, b)| **a == *b)
            .count()
            == s.chars().count()
            && self.pos + s.chars().count() <= self.chars.len()
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn consume_while<F: Fn(char) -> bool>(&mut self, test: F) -> String {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if test(c) {
                out.push(c);
                self.pos += 1;
            } else {
                break;
            }
        }
        out
    }

    /// 커서를 주어진 문자열 '다음'으로 옮깁니다(찾으면 그 뒤로, 못 찾으면 끝으로).
    fn skip_past(&mut self, s: &str) {
        while !self.eof() {
            if self.starts_with(s) {
                self.pos += s.chars().count();
                return;
            }
            self.pos += 1;
        }
    }

    /// 형제 노드들을 연달아 파싱합니다. 닫는 태그(</)나 끝을 만나면 멈춥니다.
    fn parse_nodes(&mut self) -> Vec<Node> {
        let mut nodes = Vec::new();
        loop {
            if self.eof() || self.starts_with("</") {
                break;
            }
            if self.starts_with("<!--") {
                self.skip_past("-->");
                continue;
            }
            if self.starts_with("<!") {
                self.skip_past(">"); // DOCTYPE 등.
                continue;
            }
            if self.peek() == Some('<') {
                if let Some(node) = self.parse_element() {
                    nodes.push(node);
                }
            } else {
                // 텍스트: 다음 '<' 전까지. 공백만 있으면 버립니다.
                let raw = self.consume_while(|c| c != '<');
                let text = decode_entities(raw.trim());
                if !text.is_empty() {
                    nodes.push(text_node(&text));
                }
            }
        }
        nodes
    }

    /// 요소 하나(<tag ...> ... </tag> 또는 <tag .../>)를 파싱합니다.
    fn parse_element(&mut self) -> Option<Node> {
        // 여는 '<'.
        if self.peek() != Some('<') {
            return None;
        }
        self.pos += 1;

        let tag = self.consume_while(is_name_char).to_ascii_lowercase();
        if tag.is_empty() {
            // '<' 뒤에 태그 이름이 없으면 깨진 태그 → '>'까지 버리고 무시.
            self.skip_past(">");
            return None;
        }

        let mut element = Node::new(&tag);
        let classes = self.parse_attributes();
        element.classes = classes;

        self.skip_whitespace();

        // self-closing: <tag/>
        if self.starts_with("/>") {
            self.pos += 2;
            return Some(element);
        }
        // 여는 태그 끝 '>'.
        if self.peek() == Some('>') {
            self.pos += 1;
        }
        // void 태그는 닫는 태그 없이 끝납니다.
        if VOID_TAGS.contains(&tag.as_str()) {
            return Some(element);
        }

        // 자식들을 파싱한 뒤 닫는 태그를 소비합니다.
        let children = self.parse_nodes();
        self.skip_close_tag();

        // 자식이 '글자뿐'이면 텍스트를 요소 자체에 붙입니다(그 요소의 CSS가 적용되도록).
        let has_element = children.iter().any(|c| c.tag != TEXT_TAG);
        if !has_element {
            let combined = children
                .iter()
                .filter_map(|c| c.text.clone())
                .collect::<Vec<_>>()
                .join(" ");
            if !combined.is_empty() {
                element.text = Some(combined);
            }
        } else {
            element.children = children;
        }

        Some(element)
    }

    /// 닫는 태그 `</tag>`를 너그럽게 소비합니다(이름 일치는 강제하지 않음).
    fn skip_close_tag(&mut self) {
        if self.starts_with("</") {
            self.skip_past(">");
        }
    }

    /// 속성들을 파싱해 class 값(공백으로 나눈 클래스 목록)만 모읍니다.
    fn parse_attributes(&mut self) -> Vec<String> {
        let mut classes = Vec::new();
        loop {
            self.skip_whitespace();
            match self.peek() {
                Some('>') | None => break,
                Some('/') => break, // self-closing의 '/'.
                _ => {}
            }
            let name = self.consume_while(is_name_char).to_ascii_lowercase();
            if name.is_empty() {
                // 알 수 없는 글자는 하나 건너뛰어 무한루프 방지.
                self.pos += 1;
                continue;
            }
            self.skip_whitespace();
            let mut value = String::new();
            if self.peek() == Some('=') {
                self.pos += 1;
                self.skip_whitespace();
                value = self.parse_attr_value();
            }
            if name == "class" {
                classes = value
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect();
            }
        }
        classes
    }

    /// 속성 값을 파싱합니다. "..." 또는 '...' 또는 따옴표 없는 값.
    fn parse_attr_value(&mut self) -> String {
        match self.peek() {
            Some(q @ ('"' | '\'')) => {
                self.pos += 1; // 여는 따옴표.
                let v = self.consume_while(|c| c != q);
                if self.peek() == Some(q) {
                    self.pos += 1; // 닫는 따옴표.
                }
                decode_entities(&v)
            }
            _ => {
                // 따옴표 없는 값: 공백이나 '>' 전까지.
                let v = self.consume_while(|c| !c.is_whitespace() && c != '>' && c != '/');
                decode_entities(&v)
            }
        }
    }
}

/// 텍스트 노드를 만듭니다. tag = "#text", 글자 내용만 가집니다.
fn text_node(text: &str) -> Node {
    let mut n = Node::new(TEXT_TAG);
    n.text = Some(text.to_string());
    n
}

/// 이름(태그/속성)에 쓸 수 있는 글자: 영숫자, '-', '_'.
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// 자주 쓰는 HTML 엔티티를 실제 글자로 바꿉니다.
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_elements_and_classes() {
        let root = parse(r#"<div class="card"><p class="a b">hi</p></div>"#);
        assert_eq!(root.tag, "div");
        assert_eq!(root.classes, vec!["card"]);
        assert_eq!(root.children.len(), 1);
        let p = &root.children[0];
        assert_eq!(p.tag, "p");
        assert_eq!(p.classes, vec!["a", "b"]);
        assert_eq!(p.text.as_deref(), Some("hi"));
    }

    #[test]
    fn text_only_element_keeps_text_on_itself() {
        let root = parse(r#"<span class="x">hello world</span>"#);
        assert_eq!(root.tag, "span");
        assert!(root.children.is_empty());
        assert_eq!(root.text.as_deref(), Some("hello world"));
    }

    #[test]
    fn wraps_multiple_roots_and_skips_comments() {
        let root = parse("<!-- 주석 --><a></a><b></b>");
        assert_eq!(root.tag, "div"); // 익명 래퍼.
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].tag, "a");
        assert_eq!(root.children[1].tag, "b");
    }

    #[test]
    fn handles_self_closing_and_void_and_entities() {
        let root = parse("<div><br><img class=\"logo\"/>5 &lt; 10</div>");
        // 자식: br, img, 그리고 텍스트 "5 < 10" (#text).
        assert_eq!(root.children.len(), 3);
        assert_eq!(root.children[0].tag, "br");
        assert_eq!(root.children[1].tag, "img");
        assert_eq!(root.children[1].classes, vec!["logo"]);
        assert_eq!(root.children[2].tag, TEXT_TAG);
        assert_eq!(root.children[2].text.as_deref(), Some("5 < 10"));
    }
}
