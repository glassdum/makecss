//! 페인트: 레이아웃된 박스들을 실제 픽셀로 칠합니다. = "직접 픽셀 렌더링".
//!
//! 화면은 거대한 모눈종이입니다. 칸 하나가 픽셀 하나. "렌더링"이란 결국
//! 이 모눈종이(숫자 배열)의 각 칸에 색을 적어넣는 일입니다. 거창한 마법이 아니라
//! 메모리에 숫자를 쓰는 것뿐입니다.
//!
//! 픽셀 형식: u32 하나에 0x00RRGGBB. 위 8비트는 안 씁니다(창에 띄우는 softbuffer 형식).

use crate::color::Color;
use crate::layout::{LayoutBox, Rect};

/// 픽셀들의 모눈종이. 폭 x 높이 칸을 가지며, 각 칸은 u32 색.
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    /// 길이 = width * height. (x, y) 픽셀은 인덱스 y*width + x 에 있습니다.
    /// 비유: 모눈종이를 한 줄씩 쭉 펴서 일렬로 늘어놓은 긴 띠.
    pub pixels: Vec<u32>,
}

impl Canvas {
    /// 주어진 크기의 캔버스를 배경색으로 가득 채워 만듭니다.
    pub fn new(width: u32, height: u32, background: Color) -> Self {
        let bg = pack(background);
        Canvas {
            width,
            height,
            pixels: vec![bg; (width as usize) * (height as usize)],
        }
    }

    /// (x, y) 픽셀 하나에 색을 칠합니다. 반투명이면 아래 색과 섞습니다.
    fn blend_pixel(&mut self, x: i32, y: i32, color: Color) {
        // 화면 밖 좌표는 무시(모눈종이 바깥엔 칠하지 않음).
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        if color.a == 0 {
            return; // 완전 투명이면 그릴 게 없음.
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        if color.a == 255 {
            // 완전 불투명이면 그냥 덮어쓰기.
            self.pixels[idx] = pack(color);
        } else {
            // 반투명이면 기존 색과 알파 비율로 섞기.
            let below = unpack(self.pixels[idx]);
            self.pixels[idx] = pack(blend(below, color));
        }
    }

    /// 사각형 영역을 한 색으로 채웁니다. 페인트의 기본 붓질.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        if color.a == 0 {
            return;
        }
        // 실수 좌표를 픽셀 격자에 맞춰 반올림하고, 화면 범위로 자릅니다.
        let x0 = rect.x.round().max(0.0) as i32;
        let y0 = rect.y.round().max(0.0) as i32;
        let x1 = ((rect.x + rect.width).round() as i32).min(self.width as i32);
        let y1 = ((rect.y + rect.height).round() as i32).min(self.height as i32);
        for y in y0..y1 {
            for x in x0..x1 {
                self.blend_pixel(x, y, color);
            }
        }
    }
}

/// 레이아웃 박스 트리 전체를 캔버스에 그립니다.
/// 부모를 먼저, 자식을 나중에 그립니다 → 자식이 부모 위에 자연스레 얹힙니다.
/// (화가가 배경을 먼저 칠하고 그 위에 인물을 그리는 것과 같음.)
pub fn paint(canvas: &mut Canvas, root: &LayoutBox) {
    paint_box(canvas, root);
}

fn paint_box(canvas: &mut Canvas, b: &LayoutBox) {
    let bb = b.border_box;

    // 1) 테두리: 먼저 테두리 색으로 border-box 전체를 칠합니다.
    if b.style.border_width > 0.0 && b.style.border_color.a > 0 {
        canvas.fill_rect(bb, b.style.border_color);
    }

    // 2) 배경: 테두리 안쪽(padding-box) 영역을 배경색으로 칠합니다.
    //    테두리 위에 배경을 얹으면, 테두리는 가장자리에만 남습니다.
    if b.style.background.a > 0 {
        let inner = inset(bb, b.style.border_width);
        canvas.fill_rect(inner, b.style.background);
    }

    // 3) 자식들을 그 위에 그립니다.
    for child in &b.children {
        paint_box(canvas, child);
    }
}

/// 사각형을 사방으로 d 만큼 안쪽으로 줄입니다(테두리 두께만큼 들어가기).
fn inset(r: Rect, d: f32) -> Rect {
    Rect {
        x: r.x + d,
        y: r.y + d,
        width: (r.width - 2.0 * d).max(0.0),
        height: (r.height - 2.0 * d).max(0.0),
    }
}

/// Color(0~255 채널)를 화면 픽셀 형식 0x00RRGGBB 로 압축.
fn pack(c: Color) -> u32 {
    ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32)
}

/// 0x00RRGGBB 픽셀을 다시 Color로 풉니다(불투명으로 간주).
fn unpack(p: u32) -> Color {
    Color::rgb(((p >> 16) & 0xff) as u8, ((p >> 8) & 0xff) as u8, (p & 0xff) as u8)
}

/// 아래 색 위에 위 색(top, 반투명)을 얹은 결과 색을 계산합니다.
/// 공식: 결과 = 위색 * a + 아래색 * (1 - a).  (알파 합성, "over" 연산)
/// 비유: 색 셀로판지를 겹쳐 비춰보는 것.
fn blend(below: Color, top: Color) -> Color {
    let a = top.a as f32 / 255.0;
    let mix = |t: u8, b: u8| ((t as f32) * a + (b as f32) * (1.0 - a)).round() as u8;
    Color::rgb(mix(top.r, below.r), mix(top.g, below.g), mix(top.b, below.b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_within_bounds() {
        let mut canvas = Canvas::new(4, 4, Color::WHITE);
        canvas.fill_rect(
            Rect { x: 1.0, y: 1.0, width: 2.0, height: 2.0 },
            Color::rgb(255, 0, 0),
        );
        // (1,1) 은 빨강, (0,0) 은 그대로 흰색이어야 합니다.
        assert_eq!(canvas.pixels[1 * 4 + 1], pack(Color::rgb(255, 0, 0)));
        assert_eq!(canvas.pixels[0], pack(Color::WHITE));
    }

    #[test]
    fn half_transparent_blends() {
        // 흰 배경 위에 50% 검정 → 회색(약 128).
        let mut canvas = Canvas::new(1, 1, Color::WHITE);
        canvas.fill_rect(
            Rect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
            Color::rgba(0, 0, 0, 128),
        );
        let result = unpack(canvas.pixels[0]);
        assert!((result.r as i32 - 128).abs() <= 1);
    }
}
