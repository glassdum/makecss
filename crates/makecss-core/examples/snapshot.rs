//! 화면 없이도 렌더 결과를 눈으로 확인하기 위한 예제.
//! 한 장면을 엔진으로 그려 BMP 이미지로 저장합니다.
//!
//! 실행(기본 비트맵 폰트):
//!   cargo run -p makecss-core --example snapshot
//!   → snapshot.bmp 생성
//!
//! 실행(진짜 TTF 폰트도 함께):
//!   cargo run -p makecss-core --example snapshot -- /경로/폰트.ttf
//!   (또는 환경변수 MAKECSS_FONT=/경로/폰트.ttf)
//!   → snapshot_ttf.bmp 도 추가 생성
//!
//! 이 장면은 4가지를 한눈에 보여줍니다: 소문자 · 자동 줄나눔 · 정렬 · (선택)TTF.

use std::fs::File;
use std::io::{BufWriter, Write};

use makecss_core::truetype::TtfFont;
use makecss_core::{render_html, render_html_with_font, Canvas};

const WIDTH: u32 = 540;
const HEIGHT: u32 = 360;

/// 보여줄 화면을 'HTML 문자열'로 작성합니다.
/// flex(네비게이션·카드 줄), absolute(배지), 중첩 flex를 한 화면에서 보여줍니다.
fn html() -> &'static str {
    r#"
    <div class="page">
        <div class="nav">
            <div class="brand">makecss</div>
            <div class="links">
                <div class="link">home</div>
                <div class="link">docs</div>
                <div class="link">about</div>
            </div>
        </div>
        <div class="cards">
            <div class="card">
                <div class="badge">new</div>
                <div class="h">flex grow</div>
                <div class="p">three cards share the row width equally.</div>
            </div>
            <div class="card">
                <div class="h">align</div>
                <div class="p">stretch makes every card the same height even with more text inside.</div>
            </div>
            <div class="card">
                <div class="h">absolute</div>
                <div class="p">the pink badge floats at the top-right corner.</div>
            </div>
        </div>
    </div>
    "#
}

/// 화면을 꾸미는 CSS. display / position / flex 속성들을 사용합니다.
fn css() -> &'static str {
    r#"
        .page   { background: #eef2f7; padding: 16px; }

        .nav    { display: flex; justify-content: space-between; align-items: center;
                  background: #1a1a2e; padding: 12px; }
        .brand  { color: white; font-size: 24px; }
        .links  { display: flex; gap: 16px; }
        .link   { color: #aab4d4; font-size: 16px; }

        .cards  { display: flex; align-items: stretch; gap: 12px; margin: 14px; }
        .card   { flex: 1; position: relative; background: white;
                  border-width: 1px; border-color: #ccccdd; padding: 12px; }
        .h      { font-size: 20px; color: #1a1a2e; }
        .p      { font-size: 14px; color: #445566; }
        .badge  { position: absolute; top: 8px; right: 8px;
                  background: #e0457b; color: white; font-size: 12px; padding: 4px; }
    "#
}

fn main() {
    let (html, css) = (html(), css());

    // 1) 기본 비트맵 폰트로 렌더 → snapshot.bmp
    let canvas = render_html(html, css, WIDTH, HEIGHT);
    write_bmp(&canvas, "snapshot.bmp").expect("BMP 저장 실패");
    println!("저장: snapshot.bmp ({WIDTH}x{HEIGHT}) — 비트맵 폰트");

    // 2) 폰트 경로가 주어지면 진짜 TTF로도 렌더 → snapshot_ttf.bmp
    let font_path = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("MAKECSS_FONT").ok());
    if let Some(path) = font_path {
        match std::fs::read(&path) {
            Ok(bytes) => match TtfFont::from_bytes(bytes) {
                Ok(font) => {
                    let canvas = render_html_with_font(html, css, WIDTH, HEIGHT, &font);
                    write_bmp(&canvas, "snapshot_ttf.bmp").expect("BMP 저장 실패");
                    println!("저장: snapshot_ttf.bmp ({WIDTH}x{HEIGHT}) — TTF: {path}");
                }
                Err(e) => eprintln!("TTF 파싱 실패: {e}"),
            },
            Err(e) => eprintln!("폰트 파일 읽기 실패({path}): {e}"),
        }
    }
}

/// Canvas(0x00RRGGBB 픽셀들)를 24비트 BMP 파일로 씁니다(압축 없는 단순 형식).
fn write_bmp(canvas: &Canvas, path: &str) -> std::io::Result<()> {
    let w = canvas.width as i32;
    let h = canvas.height as i32;
    let row_bytes = (w * 3 + 3) & !3; // 각 줄을 4바이트 배수로.
    let pixel_data_size = row_bytes * h;
    let file_size = 14 + 40 + pixel_data_size;

    let file = File::create(path)?;
    let mut out = BufWriter::new(file);

    // 파일 헤더(14B)
    out.write_all(b"BM")?;
    out.write_all(&(file_size as u32).to_le_bytes())?;
    out.write_all(&0u32.to_le_bytes())?;
    out.write_all(&(14u32 + 40).to_le_bytes())?;
    // 정보 헤더(40B)
    out.write_all(&40u32.to_le_bytes())?;
    out.write_all(&w.to_le_bytes())?;
    out.write_all(&h.to_le_bytes())?; // 양수 = 아래→위 저장.
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&24u16.to_le_bytes())?;
    out.write_all(&0u32.to_le_bytes())?;
    out.write_all(&(pixel_data_size as u32).to_le_bytes())?;
    out.write_all(&2835u32.to_le_bytes())?;
    out.write_all(&2835u32.to_le_bytes())?;
    out.write_all(&0u32.to_le_bytes())?;
    out.write_all(&0u32.to_le_bytes())?;

    // 픽셀: 맨 아래 줄부터, 색 순서 B,G,R.
    let pad = vec![0u8; (row_bytes - w * 3) as usize];
    for y in (0..h).rev() {
        for x in 0..w {
            let p = canvas.pixels[(y * w + x) as usize];
            let r = ((p >> 16) & 0xff) as u8;
            let g = ((p >> 8) & 0xff) as u8;
            let b = (p & 0xff) as u8;
            out.write_all(&[b, g, r])?;
        }
        out.write_all(&pad)?;
    }
    out.flush()?;
    Ok(())
}
