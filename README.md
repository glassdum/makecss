# makecss

**CSS 문법으로 네이티브 데스크톱 UI(앱·exe)를 그리는 엔진** — 직접 픽셀 렌더링 방식.

브라우저 없이, CSS 텍스트를 받아 화면 픽셀을 직접 칠합니다. 코어는 외부 의존성이
전혀 없는 순수 Rust라, 모든 단계를 들여다보며 배우기 좋게 만들었습니다.

---

## 한눈에 보는 동작 원리 ("요리 주방" 비유)

엔진은 주방입니다. CSS라는 *레시피*를 받아 픽셀이라는 *요리*를 내놓습니다.
네 명의 요리사가 순서대로 일합니다:

```text
CSS 텍스트         1.파서       2.스타일      3.레이아웃     4.페인트      픽셀
"card { ... }"  → (글자→데이터)→(누가뭘입나)→(어디에놓나)→(실제로그림)→ 화면
   레시피           재료손질      간맞추기      접시담기       불에굽기      요리
```

| 단계 | 모듈 | 하는 일 | 핵심 CSS 개념 |
|------|------|---------|---------------|
| 1. 파서   | `css.rs`    | CSS 텍스트 → 규칙 구조 | 셀렉터, 선언 |
| 2. 스타일 | `style.rs`  | 규칙 → 요소별 최종 스타일 | 캐스케이드, 명시도 |
| 3. 레이아웃 | `layout.rs` | 스타일 → 위치/크기 | 박스 모델, 블록 흐름 |
| 4. 페인트 | `paint.rs`  | 박스 → 픽셀 | 래스터화, 알파 합성 |

글자는 두 가지 폰트로 그릴 수 있습니다(둘 다 같은 `FontFace` 약속 구현):
- `font.rs` — 손수 그린 5x7 비트맵 폰트(파일 불필요, **기본값**)
- `truetype.rs` — 진짜 `.ttf` 외곽선을 직접 파싱·래스터화(안티앨리어싱)

줄나눔·정렬은 `text.rs`가 담당합니다.

코어(주방)는 창 띄우기·마우스 같은 바깥세상을 전혀 모릅니다. 그건 `makecss-demo`
(서빙 직원)가 담당합니다. 나중에 Python/Java/C# 바인딩도 이 "서빙" 자리에 들어옵니다.

## 구조

```text
makecss/
├─ crates/
│  ├─ makecss-core/      # 엔진. 의존성 0. CSS → 픽셀.
│  │  └─ src/
│  │     ├─ color.rs     # 색 타입 + CSS 색 파싱
│  │     ├─ dom.rs       # 문서 트리(요소들)
│  │     ├─ css.rs       # 1) CSS 파서
│  │     ├─ style.rs     # 2) 스타일 매칭/캐스케이드
│  │     ├─ layout.rs    # 3) 박스 모델 레이아웃
│  │     ├─ paint.rs     # 4) 직접 픽셀 렌더링(Canvas)
│  │     ├─ fontface.rs  # 폰트 공통 인터페이스(FontFace) + 글자 도장
│  │     ├─ font.rs      # 5x7 비트맵 폰트(기본)
│  │     ├─ truetype.rs  # 진짜 .ttf 파서 + 래스터화(안티앨리어싱)
│  │     ├─ text.rs      # 자동 줄나눔(word wrap) + 정렬(text-align)
│  │     ├─ html.rs      # HTML 파서: 마크업 → Node 트리
│  │     └─ lib.rs       # 정문: render() / render_html() / *_with_font()
│  │  └─ examples/
│  │     └─ snapshot.rs  # 장면을 BMP 이미지로 저장(화면 없이 결과 확인)
│  └─ makecss-demo/      # winit으로 창 띄우고 픽셀을 보여주는 쇼룸
└─ Cargo.toml            # 워크스페이스
```

## 실행

```bash
# 코어 단위 테스트 (디스플레이 불필요)
cargo test -p makecss-core

# 창 띄워 실제로 보기 (디스플레이 있는 로컬 PC에서)
cargo run -p makecss-demo

# 화면 없이 결과를 이미지로 저장 (snapshot.bmp 생성)
cargo run -p makecss-core --example snapshot

# 진짜 TTF 폰트로도 저장 (snapshot_ttf.bmp 추가 생성)
cargo run -p makecss-core --example snapshot -- /경로/폰트.ttf

# 창 데모에서 TTF 쓰기
MAKECSS_FONT=/경로/폰트.ttf cargo run -p makecss-demo
```

## 가장 작은 예제

```rust
use makecss_core::render_html;

let html = r#"<div class="card">Hello, makecss!</div>"#;
let css  = r#".card { background: #eef; padding: 12px; font-size: 20px; }"#;

let canvas = render_html(html, css, 320, 80); // canvas.pixels = Vec<u32> (0x00RRGGBB)
// 진짜 폰트로: render_html_with_font(html, css, w, h, &TtfFont::from_bytes(bytes)?)
```

## 지금 지원하는 CSS (MVP)

- **셀렉터**: `*`, 태그(`div`), 클래스(`.card`)
- **속성**: `width`, `height`, `padding`, `margin`, `border-width`, `border-color`,
  `background`/`background-color`, `color`, `font-size`, `text-align`
- **값**: `px` 길이, `#rgb`/`#rrggbb`/`rgb()`/`rgba()`/색 이름
- **배치**: 블록 흐름(자식을 위→아래로 쌓기)
- **텍스트**: 대문자·소문자·숫자·문장부호, **자동 줄나눔**, **정렬**(left/center/right),
  비트맵 폰트 또는 **진짜 TTF 폰트**(안티앨리어싱). 세로 정렬·여러 글꼴 혼용은 아직 없음.
- **HTML**: `<div class="card">...</div>` 마크업 → Node 트리. 중첩, `class` 속성,
  self-closing/void 태그(`<br>`), 주석, 기본 엔티티 지원. 스타일 상속·`id`는 아직 없음.

## 로드맵 (다음 단계)

1. ✅ **텍스트 렌더링** — 비트맵 폰트, 소문자, 자동 줄나눔, 정렬, **TTF 폰트**. *(완료)*
2. ✅ **HTML 파서** — `<div class="card">...</div>` 마크업으로 화면 작성. *(완료)*
3. **레이아웃 강화** — Flexbox, 퍼센트/`em` 단위, 변마다 다른 padding/margin, 스타일 상속
4. **인터랙션** — 마우스/키보드 이벤트, `:hover`/`:focus`, 버튼·입력창
5. **멀티언어 바인딩** — C ABI 노출 → Python(ctypes)/Java(JNI)/C#(P/Invoke)
6. **실사용 다듬기** — 줄바꿈(`\n`)·세로 정렬·글꼴 캐싱, 애니메이션, 고DPI, 패키징
