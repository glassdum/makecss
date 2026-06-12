//! makecss 쇼룸: 엔진(makecss-core)이 만든 픽셀을 실제 OS 창에 띄웁니다.
//!
//! 이 파일이 하는 일은 딱 셋:
//!   1) winit으로 창을 하나 띄운다.
//!   2) 창을 다시 그려야 할 때마다, 엔진에게 "이 CSS로 이 요소들을 창 크기에 그려줘"
//!      라고 주문해 픽셀(Canvas)을 받는다.
//!   3) softbuffer로 그 픽셀을 창에 그대로 붙인다.
//!
//! 즉 'UI를 어떻게 그릴지'는 전부 코어가 하고, 여기는 '창과의 연결'만 담당합니다.
//! 나중에 Python/Java/C# 바인딩이 하는 일도 본질적으로 이것과 같습니다.
//!
//! 참고: 실행하려면 화면(디스플레이)이 있는 환경이어야 합니다. 헤드리스 서버에서는
//! 빌드(cargo build)는 되지만 실행 시 창을 못 띄웁니다. 로컬 PC에서 `cargo run`.

use std::num::NonZeroU32;
use std::rc::Rc;

use makecss_core::truetype::TtfFont;
use makecss_core::{render, render_with_font, Node};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// 화면에 보여줄 '문서'와 'CSS'를 만듭니다.
/// 지금은 HTML 파서가 없으니 요소 트리를 코드로 직접 조립합니다(다음 단계에서 HTML 지원).
/// 소문자 · 자동 줄나눔 · 정렬을 한 화면에서 보여줍니다.
fn scene() -> (Node, String) {
    let root = Node::new("div")
        .class("page")
        .child(Node::new("div").class("title").text("makecss demo"))
        .child(
            Node::new("div").class("card").child(
                Node::new("div").class("para").text(
                    "The quick brown fox jumps over the lazy dog. \
                     This sentence wraps automatically to fit the card.",
                ),
            ),
        )
        .child(Node::new("div").class("bar").text("left aligned"))
        .child(Node::new("div").class("bar").class("c").text("center aligned"))
        .child(Node::new("div").class("bar").class("r").text("right aligned"));

    let css = r#"
        .page  { background: #eef2f7; padding: 16px; }
        .title { font-size: 28px; color: #1a1a2e; padding: 4px; }
        .card  { background: white; border-width: 1px; border-color: #cccccc;
                 padding: 12px; margin: 8px; color: #333344; }
        .para  { font-size: 16px; }
        .bar   { background: #dde6f0; color: #223355; font-size: 16px;
                 padding: 6px; margin: 6px; }
        .c     { text-align: center; }
        .r     { text-align: right; }
    "#;

    (root, css.to_string())
}

/// 앱 상태: 창, 픽셀을 붙일 표면(surface), 그리고 선택적 TTF 폰트.
/// 환경변수 MAKECSS_FONT=/경로/폰트.ttf 를 주면 진짜 폰트로 그립니다(없으면 비트맵).
struct App {
    window: Option<Rc<Window>>,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    font: Option<TtfFont>,
}

impl ApplicationHandler for App {
    /// 앱이 시작/재개될 때: 창을 만들고 softbuffer 표면을 준비합니다.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attributes = Window::default_attributes()
            .with_title("makecss demo")
            .with_inner_size(LogicalSize::new(420.0, 320.0));
        let window = Rc::new(event_loop.create_window(attributes).unwrap());

        // softbuffer: 창과 연결된 그리기 표면 준비.
        let context = Context::new(window.clone()).unwrap();
        let surface = Surface::new(&context, window.clone()).unwrap();

        window.request_redraw(); // 첫 그림 요청.
        self.window = Some(window);
        self.surface = Some(surface);
    }

    /// 창에서 일어나는 사건들(닫기 버튼, 다시 그리기 요청 등) 처리.
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            // 창 닫기 버튼 → 프로그램 종료.
            WindowEvent::CloseRequested => event_loop.exit(),

            // "다시 그려야 함" → 엔진에게 픽셀을 받아 창에 붙입니다.
            WindowEvent::RedrawRequested => {
                let (Some(window), Some(surface)) =
                    (self.window.as_ref(), self.surface.as_mut())
                else {
                    return;
                };

                let size = window.inner_size();
                let (w, h) = (size.width, size.height);
                if w == 0 || h == 0 {
                    return; // 창이 최소화되면 크기가 0이 될 수 있음.
                }

                // 표면 크기를 창 크기에 맞춥니다.
                surface
                    .resize(NonZeroU32::new(w).unwrap(), NonZeroU32::new(h).unwrap())
                    .unwrap();

                // ── 여기가 핵심: 엔진을 호출해 픽셀을 얻습니다. ──
                // TTF 폰트가 로드돼 있으면 그것으로, 아니면 내장 비트맵 폰트로.
                let (root, css) = scene();
                let canvas = match &self.font {
                    Some(font) => render_with_font(&root, &css, w, h, font),
                    None => render(&root, &css, w, h),
                };

                // 얻은 픽셀을 창 표면 버퍼에 그대로 복사한 뒤 화면에 띄웁니다.
                let mut buffer = surface.buffer_mut().unwrap();
                buffer.copy_from_slice(&canvas.pixels);
                buffer.present().unwrap();
            }
            _ => {}
        }
    }
}

fn main() {
    // 선택: 환경변수 MAKECSS_FONT 에 .ttf 경로가 있으면 진짜 폰트로 그립니다.
    let font = std::env::var("MAKECSS_FONT").ok().and_then(|path| {
        match std::fs::read(&path).map(TtfFont::from_bytes) {
            Ok(Ok(font)) => {
                println!("TTF 폰트 사용: {path}");
                Some(font)
            }
            Ok(Err(e)) => {
                eprintln!("TTF 파싱 실패({path}): {e} — 비트맵 폰트로 대체");
                None
            }
            Err(e) => {
                eprintln!("폰트 읽기 실패({path}): {e} — 비트맵 폰트로 대체");
                None
            }
        }
    });

    let event_loop = EventLoop::new().unwrap();
    // Wait: 할 일이 없으면 CPU를 쉬게 합니다(이벤트가 올 때만 깨어남).
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App {
        window: None,
        surface: None,
        font,
    };
    event_loop.run_app(&mut app).unwrap();
}
