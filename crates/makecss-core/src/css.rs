//! CSS 파서: 글자 덩어리인 CSS 텍스트를 구조화된 데이터로 바꿉니다.
//!
//! 입력 예시:
//! ```text
//!     .card { background: #eee; padding: 10px; }
//!     div   { width: 200px; }
//! ```
//!
//! 출력은 아래 구조들의 트리입니다:
//! ```text
//!     Stylesheet(스타일시트) = Rule(규칙)들의 목록
//!     Rule(규칙)             = 셀렉터들 + 선언들  (".card { ... }")
//!     Selector(셀렉터)       = 누구에게 적용할지   (.card / div / *)
//!     Declaration(선언)      = 속성: 값 한 쌍      (padding: 10px)
//! ```
//!
//! 비유: 받아쓰기한 문장을 듣고 표로 정리하는 일. 파서는 글자를 왼쪽부터
//! 한 글자씩 읽으며("커서"를 옮기며) 표의 칸을 하나씩 채웁니다.

/// CSS 전체 = 규칙들의 목록.
#[derive(Debug, Clone)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

/// 규칙 하나 = 여러 셀렉터 + 여러 선언.
/// 예: `h1, .title { color: red; }` 는 셀렉터 2개, 선언 1개.
#[derive(Debug, Clone)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
}

/// 단순 셀렉터 = "어떤 요소인가"(상태는 빼고).
#[derive(Debug, Clone, PartialEq)]
pub enum SimpleSelector {
    /// `*` : 모든 요소.
    Universal,
    /// `div` : 그 태그 이름을 가진 요소.
    Type(String),
    /// `.card` : 그 클래스를 가진 요소.
    Class(String),
}

/// 셀렉터 = 단순 셀렉터 + (선택) 상태 가상클래스.
/// 예: `.btn:hover` = 클래스 btn 이면서 마우스가 위에 있을 때.
#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    pub simple: SimpleSelector,
    /// `:hover` 가 붙었는지. true면 마우스가 위에 있을 때만 적용.
    pub hover: bool,
}

impl Selector {
    /// 상태 없는 단순 셀렉터를 만듭니다(테스트/편의용).
    pub fn simple(simple: SimpleSelector) -> Self {
        Selector { simple, hover: false }
    }
}

/// 선언 = "속성: 값" 한 쌍. 예: padding 라는 속성에 "10px" 라는 값.
#[derive(Debug, Clone)]
pub struct Declaration {
    pub property: String,
    pub value: String,
}

/// CSS 텍스트를 파싱해 Stylesheet를 만듭니다. 이것이 이 모듈의 공개 입구.
pub fn parse(input: &str) -> Stylesheet {
    let mut parser = Parser {
        chars: input.chars().collect(),
        pos: 0,
    };
    let mut rules = Vec::new();
    loop {
        parser.skip_whitespace();
        if parser.eof() {
            break;
        }
        let before = parser.pos;
        if let Some(rule) = parser.parse_rule() {
            rules.push(rule);
        } else {
            // 알 수 없거나 깨진 구문(예: @media, 떠도는 '}')은 통째로 버리지 않고
            // 그 블록만 건너뛰고 계속합니다(브라우저처럼). 다음 '}' 뒤로 이동.
            parser.recover_to_next_block();
        }
        // 진행이 없으면 한 글자 강제로 넘겨 무한 루프를 막습니다.
        if parser.pos == before {
            parser.pos += 1;
        }
    }
    Stylesheet { rules }
}

/// 파서의 상태: 글자 배열 + 지금 보고 있는 위치(커서).
/// 비유: 책을 읽는 손가락. pos가 손가락이 가리키는 글자 위치입니다.
struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    /// 커서가 글의 끝에 도달했는가?
    fn eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    /// 커서가 가리키는 글자를 '소비하지 않고' 살짝 엿봅니다(peek = 엿보기).
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    /// 커서가 가리키는 글자를 하나 먹고(소비) 커서를 앞으로 옮깁니다.
    fn next(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    /// 공백/줄바꿈을 건너뜁니다. CSS에서 공백은 보통 의미가 없으니까요.
    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// 조건을 만족하는 글자들을 모아 하나의 문자열로 가져옵니다.
    /// 예: "이름에 쓸 수 있는 글자들"을 연속으로 긁어모으기.
    fn consume_while<F: Fn(char) -> bool>(&mut self, test: F) -> String {
        let mut result = String::new();
        while let Some(c) = self.peek() {
            if test(c) {
                result.push(c);
                self.pos += 1;
            } else {
                break;
            }
        }
        result
    }

    /// 깨졌거나 지원하지 않는 구문에서 회복합니다.
    /// - '@'로 시작하는 at-규칙(@media 등)은 블록(또는 ';')까지 통째로 건너뜁니다.
    /// - 그 밖의 예기치 못한 글자는 '하나만' 건너뛰어, 뒤따르는 정상 규칙을 지킵니다.
    fn recover_to_next_block(&mut self) {
        if self.peek() == Some('@') {
            while let Some(c) = self.peek() {
                self.pos += 1;
                if c == ';' {
                    return; // @import 처럼 블록 없이 끝나는 규칙.
                }
                if c == '{' {
                    // 중괄호 깊이를 맞춰 닫는 '}'까지 건너뜁니다(중첩 대비).
                    let mut depth = 1;
                    while let Some(d) = self.peek() {
                        self.pos += 1;
                        match d {
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    return;
                                }
                            }
                            _ => {}
                        }
                    }
                    return;
                }
            }
            return;
        }
        self.pos += 1;
    }

    /// 규칙 하나(`셀렉터들 { 선언들 }`)를 파싱합니다.
    fn parse_rule(&mut self) -> Option<Rule> {
        let selectors = self.parse_selectors();
        if selectors.is_empty() {
            return None;
        }
        let declarations = self.parse_declarations();
        Some(Rule {
            selectors,
            declarations,
        })
    }

    /// `{` 가 나오기 전까지의 셀렉터들을 콤마로 나눠 파싱합니다.
    fn parse_selectors(&mut self) -> Vec<Selector> {
        let mut selectors = Vec::new();
        loop {
            self.skip_whitespace();
            let sel = self.parse_one_selector();
            if let Some(sel) = sel {
                selectors.push(sel);
            }
            self.skip_whitespace();
            match self.peek() {
                Some(',') => {
                    self.next(); // 콤마를 먹고 다음 셀렉터로.
                }
                _ => break, // '{' 또는 끝을 만나면 셀렉터 목록 종료.
            }
        }
        selectors
    }

    /// 셀렉터 하나(`*`, `div`, `.card`, 그리고 선택적 `:hover`)를 파싱합니다.
    fn parse_one_selector(&mut self) -> Option<Selector> {
        let simple = match self.peek()? {
            '*' => {
                self.next();
                SimpleSelector::Universal
            }
            '.' => {
                self.next(); // 점을 먹고 클래스 이름을 읽습니다.
                let name = self.consume_while(is_name_char);
                if name.is_empty() {
                    return None;
                }
                SimpleSelector::Class(name)
            }
            c if is_name_char(c) => {
                let name = self.consume_while(is_name_char);
                SimpleSelector::Type(name)
            }
            _ => return None,
        };

        // 선택적 가상클래스 `:hover`. 그 외 가상클래스는 (아직) 지원하지 않아
        // 셀렉터를 무효화합니다(아무것도 매칭하지 않도록).
        let mut hover = false;
        if self.peek() == Some(':') {
            self.next();
            let pseudo = self.consume_while(is_name_char);
            if pseudo == "hover" {
                hover = true;
            } else {
                return None;
            }
        }

        Some(Selector { simple, hover })
    }

    /// `{ ... }` 안의 선언들을 파싱합니다.
    fn parse_declarations(&mut self) -> Vec<Declaration> {
        let mut declarations = Vec::new();
        self.skip_whitespace();
        // 여는 중괄호 '{' 를 먹습니다.
        if self.peek() == Some('{') {
            self.next();
        } else {
            return declarations; // 형식이 어긋나면 빈 목록.
        }

        loop {
            self.skip_whitespace();
            match self.peek() {
                Some('}') => {
                    self.next(); // 닫는 중괄호를 먹고 종료.
                    break;
                }
                None => break, // 파일 끝.
                _ => {
                    if let Some(decl) = self.parse_declaration() {
                        declarations.push(decl);
                    } else {
                        // 망가진 선언은 세미콜론까지 건너뛰고 회복합니다.
                        self.consume_while(|c| c != ';' && c != '}');
                        if self.peek() == Some(';') {
                            self.next();
                        }
                    }
                }
            }
        }
        declarations
    }

    /// 선언 하나(`property: value;`)를 파싱합니다.
    fn parse_declaration(&mut self) -> Option<Declaration> {
        self.skip_whitespace();
        let property = self.consume_while(is_name_char);
        if property.is_empty() {
            return None;
        }
        self.skip_whitespace();
        // 속성과 값 사이의 콜론 ':' 을 먹습니다.
        if self.peek() != Some(':') {
            return None;
        }
        self.next();
        self.skip_whitespace();
        // 값은 세미콜론이나 닫는 중괄호 전까지 전부.
        let value = self
            .consume_while(|c| c != ';' && c != '}')
            .trim()
            .to_string();
        // 세미콜론이 있으면 먹어줍니다(마지막 선언은 없을 수도 있음).
        if self.peek() == Some(';') {
            self.next();
        }
        Some(Declaration { property, value })
    }
}

/// 이름(태그/클래스/속성)에 쓸 수 있는 글자인가? 영문자, 숫자, '-', '_'.
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hover_pseudo_class() {
        let sheet = parse(".btn:hover { background: red; }");
        let sel = &sheet.rules[0].selectors[0];
        assert_eq!(sel.simple, SimpleSelector::Class("btn".into()));
        assert!(sel.hover);
    }

    #[test]
    fn parses_a_simple_rule() {
        let sheet = parse(".card { background: blue; padding: 10px; }");
        assert_eq!(sheet.rules.len(), 1);
        let rule = &sheet.rules[0];
        assert_eq!(rule.selectors, vec![Selector::simple(SimpleSelector::Class("card".into()))]);
        assert_eq!(rule.declarations.len(), 2);
        assert_eq!(rule.declarations[0].property, "background");
        assert_eq!(rule.declarations[0].value, "blue");
        assert_eq!(rule.declarations[1].property, "padding");
        assert_eq!(rule.declarations[1].value, "10px");
    }

    #[test]
    fn recovers_after_at_rules_and_stray_tokens() {
        // @media 블록과 떠도는 '}' 뒤에 오는 정상 규칙도 끝까지 파싱되어야 합니다.
        let sheet = parse("@media screen { .x { color: red; } } } .a { color: blue; }");
        let last = sheet.rules.last().expect("정상 규칙이 살아있어야 함");
        assert_eq!(last.selectors, vec![Selector::simple(SimpleSelector::Class("a".into()))]);
        assert_eq!(last.declarations[0].value, "blue");
    }

    #[test]
    fn parses_multiple_selectors_and_rules() {
        let sheet = parse("h1, .title { color: red; }\n* { margin: 0; }");
        assert_eq!(sheet.rules.len(), 2);
        assert_eq!(sheet.rules[0].selectors.len(), 2);
        assert_eq!(sheet.rules[1].selectors, vec![Selector::simple(SimpleSelector::Universal)]);
    }
}
