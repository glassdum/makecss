//! 화면 없이도 렌더 결과를 눈으로 확인하기 위한 예제.
//! 엔진으로 한 장면을 그린 뒤, 그 픽셀을 BMP 이미지 파일로 저장합니다.
//!
//! 실행:  cargo run -p makecss-core --example snapshot
//! 결과:  현재 폴더에 snapshot.bmp 생성.
//!
//! 참고: 엔진(코어)은 파일 저장을 하지 않습니다. '픽셀을 이미지로 굽는' 일은
//! 앱의 몫이라 이 예제 파일 안에 BMP 인코더를 직접 두었습니다(의존성 0).

use std::fs::File;
use std::io::{BufWriter, Write};

use makecss_core::{render, Canvas, Node};

fn main() {
    // 데모와 같은 장면: 회색 페이지 위에 카드 두 장, 글자 포함.
    let root = Node::new("div")
        .class("page")
        .child(
            Node::new("div")
                .class("card")
                .child(Node::new("div").class("title").text("MAKECSS"))
                .child(Node::new("div").class("accent").text("CSS TO PIXELS")),
        )
        .child(Node::new("div").class("card").text("HELLO, BOX MODEL!"));

    let css = r#"
        .page   { background: #f0f0f0; padding: 16px; }
        .card   { background: white; border-width: 1px; border-color: #cccccc;
                  padding: 12px; margin: 8px; color: #333333; }
        .title  { font-size: 28px; color: #111111; }
        .accent { background: rgba(0, 120, 255, 0.5); color: white;
                  font-size: 16px; padding: 6px; margin: 8px; }
    "#;

    let canvas = render(&root, css, 420, 260);

    let path = "snapshot.bmp";
    write_bmp(&canvas, path).expect("BMP 저장 실패");
    println!("저장 완료: {path} ({}x{})", canvas.width, canvas.height);
}

/// Canvas(0x00RRGGBB 픽셀들)를 24비트 BMP 파일로 씁니다.
/// BMP는 압축 없이 픽셀을 그대로 나열하는 단순한 형식이라 직접 만들기 쉽습니다.
fn write_bmp(canvas: &Canvas, path: &str) -> std::io::Result<()> {
    let w = canvas.width as i32;
    let h = canvas.height as i32;

    // BMP의 각 줄(row)은 4바이트 배수로 맞춰야 합니다(패딩).
    let row_bytes = (w * 3 + 3) & !3; // 3바이트/픽셀 후 4의 배수로 올림.
    let pixel_data_size = row_bytes * h;
    let file_size = 14 + 40 + pixel_data_size; // 파일헤더(14) + 정보헤더(40) + 픽셀.

    let file = File::create(path)?;
    let mut out = BufWriter::new(file);

    // ── 파일 헤더 (14바이트) ──
    out.write_all(b"BM")?; // 서명.
    out.write_all(&(file_size as u32).to_le_bytes())?;
    out.write_all(&0u32.to_le_bytes())?; // 예약.
    out.write_all(&(14u32 + 40).to_le_bytes())?; // 픽셀 데이터 시작 위치.

    // ── 정보 헤더 (40바이트, BITMAPINFOHEADER) ──
    out.write_all(&40u32.to_le_bytes())?; // 헤더 크기.
    out.write_all(&w.to_le_bytes())?;
    out.write_all(&h.to_le_bytes())?; // 양수 = 아래에서 위로 저장.
    out.write_all(&1u16.to_le_bytes())?; // 플레인 수.
    out.write_all(&24u16.to_le_bytes())?; // 픽셀당 비트 = 24.
    out.write_all(&0u32.to_le_bytes())?; // 압축 없음.
    out.write_all(&(pixel_data_size as u32).to_le_bytes())?;
    out.write_all(&2835u32.to_le_bytes())?; // 가로 해상도(72dpi).
    out.write_all(&2835u32.to_le_bytes())?; // 세로 해상도.
    out.write_all(&0u32.to_le_bytes())?; // 팔레트 색 수.
    out.write_all(&0u32.to_le_bytes())?; // 중요한 색 수.

    // ── 픽셀 데이터 ── BMP는 맨 아래 줄부터 저장하고, 색 순서는 B, G, R.
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
