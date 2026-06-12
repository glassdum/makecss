//! 페인트: 레이아웃된 박스들을 실제 픽셀로 칠합니다. = "직접 픽셀 렌더링".
//!
//! 화면은 거대한 모눈종이입니다. 칸 하나가 픽셀 하나. "렌더링"이란 결국
//! 이 모눈종이(숫자 배열)의 각 칸에 색을 적어넣는 일입니다. 거창한 마법이 아니라
//! 메모리에 숫자를 쓰는 것뿐입니다.
//!
//! 픽셀 형식: u32 하나에 0x00RRGGBB. 위 8비트는 안 씁니다(창에 띄우는 softbuffer 형식).

use crate::color::Color;
use crate::fontface::FontFace;
use crate::layout::{LayoutBox, Rect};
use crate::text;

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
    /// 글자 도장을 찍는 text 모듈에서도 쓰므로 공개합니다.
    pub fn blend_pixel(&mut self, x: i32, y: i32, color: Color) {
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
///
/// pointer: 마우스 위치(px). 어떤 박스 위에 있으면 그 박스의 :hover 스타일(색)을 씁니다.
/// 레이아웃은 그대로이고 '색만' 바뀌므로, 마우스 이동 시 재배치 없이 다시 칠하기만 하면 됩니다.
pub fn paint(canvas: &mut Canvas, root: &LayoutBox, font: &dyn FontFace, pointer: Option<(f32, f32)>) {
    paint_box(canvas, root, font, pointer);
}

/// 점 (px, py)가 사각형 안에 있는지.
fn contains(r: Rect, px: f32, py: f32) -> bool {
    px >= r.x && px < r.x + r.width && py >= r.y && py < r.y + r.height
}

fn paint_box(canvas: &mut Canvas, b: &LayoutBox, font: &dyn FontFace, pointer: Option<(f32, f32)>) {
    let bb = b.border_box;

    // 이 박스 위에 마우스가 있으면 hover 스타일(색)을 고릅니다. 없으면 기본 스타일.
    let hovered = pointer.map(|(px, py)| contains(bb, px, py)).unwrap_or(false);
    let s = if hovered {
        b.hover_style.as_ref().unwrap_or(&b.style)
    } else {
        &b.style
    };
    // 테두리 두께/패딩은 레이아웃이 정한 기본값을 써서 기하 일관성을 지키고,
    // 색(테두리색/배경/글자색)만 hover 스타일을 반영합니다.
    let edge = b.style.border_width;

    // 1) 테두리: 먼저 테두리 색으로 border-box 전체를 칠합니다.
    if edge > 0.0 && s.border_color.a > 0 {
        canvas.fill_rect(bb, s.border_color);
    }

    // 2) 배경: 테두리 안쪽(padding-box) 영역을 배경색으로 칠합니다.
    if s.background.a > 0 {
        let inner = inset(bb, edge);
        canvas.fill_rect(inner, s.background);
    }

    // 3) 글자: 이미 줄나눔/정렬이 끝난 TextBlock을 내용 영역 왼쪽 위부터 찍습니다.
    if let Some(block) = &b.text {
        let inner_x = bb.x + edge + b.style.padding;
        let inner_y = bb.y + edge + b.style.padding;
        text::paint_text(canvas, block, inner_x, inner_y, font, b.style.font_size, s.color);
    }

    // 4) 자식들을 그 위에 그립니다.
    for child in &b.children {
        paint_box(canvas, child, font, pointer);
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
        assert_eq!(canvas.pixels[4 + 1], pack(Color::rgb(255, 0, 0)));
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
