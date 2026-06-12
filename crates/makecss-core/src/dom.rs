//! 문서 트리(DOM, Document Object Model).
//!
//! CSS는 "무엇을 꾸밀지" 대상이 있어야 의미가 있습니다. 그 대상이 바로 이 트리입니다.
//! 웹에서는 HTML이 이 트리를 만들지만(<div>, <p> ...), 우리 MVP에서는
//! 아직 HTML 파서가 없으므로 Rust 코드로 직접 트리를 조립합니다.
//!
//! 비유: CSS가 '옷'이라면 Node는 '옷을 입을 사람'. 사람마다 이름표(tag)와
//! 배지(class)를 달고 있고, CSS는 그 표/배지를 보고 누구에게 무슨 옷을 입힐지 정합니다.

/// 화면 요소 하나. 트리의 한 마디(node).
#[derive(Debug, Clone)]
pub struct Node {
    /// 요소 종류 이름. 웹의 "div", "button" 같은 것.
    pub tag: String,
    /// 이 요소가 가진 클래스들. CSS의 ".card" 같은 셀렉터가 여기에 매칭됩니다.
    pub classes: Vec<String>,
    /// 이 요소 안에 들어있는 자식 요소들.
    pub children: Vec<Node>,
    /// 이 요소가 직접 담은 글자 내용. 예: <div>안녕</div> 의 "안녕".
    /// 없으면 None. MVP에서는 한 요소가 '텍스트 또는 자식들' 중 하나를 가집니다.
    pub text: Option<String>,
}

impl Node {
    /// 태그 이름만으로 빈 요소를 만듭니다.
    pub fn new(tag: &str) -> Self {
        Node {
            tag: tag.to_string(),
            classes: Vec::new(),
            children: Vec::new(),
            text: None,
        }
    }

    /// 클래스를 하나 붙입니다. 메서드를 이어서 호출할 수 있게 self를 돌려줍니다.
    /// 예: Node::new("div").class("card")
    pub fn class(mut self, name: &str) -> Self {
        self.classes.push(name.to_string());
        self
    }

    /// 자식 요소 하나를 추가합니다. 역시 이어 붙이기 가능.
    pub fn child(mut self, node: Node) -> Self {
        self.children.push(node);
        self
    }

    /// 글자 내용을 담습니다. 예: Node::new("div").text("Hello")
    pub fn text(mut self, content: &str) -> Self {
        self.text = Some(content.to_string());
        self
    }
}
