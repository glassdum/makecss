//! 텍스트 레이아웃 엔진: 자동 줄나눔(word wrap)과 정렬(text-align).
//!
//! 한 줄로 쭉 늘어놓는 게 아니라, 주어진 너비(max_width) 안에서 단어 단위로
//! 끊어 여러 줄로 배치합니다. 그리고 각 줄을 왼쪽/가운데/오른쪽으로 정렬합니다.
//!
//! 비유: 타자기. 단어를 치다가 줄 끝에 닿으면 자동으로 '다음 줄'로 넘어갑니다.
//! 정렬은 다 친 줄을 통째로 좌우로 미는 것.
//!
//! 폰트가 비트맵이든 TTF든 상관없습니다 — 글자 폭/도장을 FontFace로 물어볼 뿐입니다.

use crate::color::Color;
use crate::fontface::FontFace;
use crate::paint::Canvas;
use crate::style::TextAlign;

/// 한 줄에 배치된 글자 하나. x는 '블록 왼쪽'을 0으로 한 가로 위치(정렬 반영됨).
#[derive(Debug, Clone)]
pub struct PlacedGlyph {
    pub ch: char,
    pub x: f32,
}

/// 배치가 끝난 한 줄.
#[derive(Debug, Clone)]
pub struct TextLine {
    pub glyphs: Vec<PlacedGlyph>,
    /// 이 줄의 글자들이 차지한 가로 폭(정렬 계산에 사용).
    pub width: f32,
    /// 블록 위쪽을 0으로 한 이 줄의 세로 위치.
    pub top: f32,
}

/// 자동 줄나눔까지 끝난 텍스트 덩어리.
#[derive(Debug, Clone)]
pub struct TextBlock {
    pub lines: Vec<TextLine>,
    /// 줄나눔에 사용한 너비(보통 내용 영역 폭).
    pub width: f32,
    /// 전체 높이 = 줄 수 x 줄 높이.
    pub height: f32,
}

/// 글자열을 max_width 안에서 줄나눔하고 정렬해 TextBlock으로 만듭니다.
///
/// 알고리즘(탐욕적 줄 채우기):
///   1) 공백으로 단어를 나눕니다.
///   2) 현재 줄에 단어를 더했을 때 너비를 넘으면 → 새 줄을 시작.
///   3) 단어 하나가 통째로 너비보다 길면 → 글자 단위로 강제로 끊습니다.
///   4) 마지막에 정렬에 따라 각 줄을 좌우로 밀어줍니다.
pub fn layout_text(
    text: &str,
    font: &dyn FontFace,
    font_size: f32,
    max_width: f32,
    align: TextAlign,
) -> TextBlock {
    let space_w = font.advance(' ', font_size);
    let line_h = font.line_height(font_size);

    let mut lines: Vec<TextLine> = Vec::new();
    let mut cur: Vec<PlacedGlyph> = Vec::new();
    let mut cur_w = 0.0_f32; // 현재 줄에 쌓인 가로 폭(펜 위치).

    // 현재 줄을 마감해 lines에 넣고 비웁니다.
    let flush = |cur: &mut Vec<PlacedGlyph>, cur_w: &mut f32, lines: &mut Vec<TextLine>| {
        lines.push(TextLine {
            glyphs: std::mem::take(cur),
            width: *cur_w,
            top: 0.0,
        });
        *cur_w = 0.0;
    };

    for word in text.split_whitespace() {
        let word_w: f32 = word.chars().map(|c| font.advance(c, font_size)).sum();

        // 단어 하나가 줄 폭보다 길면: 글자 단위로 끊어 채웁니다.
        if max_width > 0.0 && word_w > max_width {
            if !cur.is_empty() {
                flush(&mut cur, &mut cur_w, &mut lines);
            }
            for c in word.chars() {
                let cw = font.advance(c, font_size);
                if !cur.is_empty() && cur_w + cw > max_width {
                    flush(&mut cur, &mut cur_w, &mut lines);
                }
                cur.push(PlacedGlyph { ch: c, x: cur_w });
                cur_w += cw;
            }
            continue;
        }

        // 일반 단어: 이 단어를 더하면 폭을 넘는지 확인.
        let with_space = if cur.is_empty() { 0.0 } else { space_w };
        if !cur.is_empty() && cur_w + with_space + word_w > max_width {
            flush(&mut cur, &mut cur_w, &mut lines);
        }
        // 줄 중간이면 단어 앞에 공백 한 칸(글자는 안 그리고 폭만 전진).
        if !cur.is_empty() {
            cur_w += space_w;
        }
        for c in word.chars() {
            cur.push(PlacedGlyph { ch: c, x: cur_w });
            cur_w += font.advance(c, font_size);
        }
    }
    // 마지막 줄(혹은 빈 텍스트의 빈 줄)을 마감.
    if !cur.is_empty() || lines.is_empty() {
        flush(&mut cur, &mut cur_w, &mut lines);
    }

    // 정렬: 남는 가로 공간을 정렬 비율만큼 각 줄에 더해 줍니다.
    let factor = match align {
        TextAlign::Left => 0.0,
        TextAlign::Center => 0.5,
        TextAlign::Right => 1.0,
    };
    for (i, line) in lines.iter_mut().enumerate() {
        let offset = (max_width - line.width).max(0.0) * factor;
        if offset != 0.0 {
            for g in &mut line.glyphs {
                g.x += offset;
            }
        }
        line.top = i as f32 * line_h;
    }

    let height = lines.len() as f32 * line_h;
    TextBlock { lines, width: max_width, height }
}

/// 배치가 끝난 TextBlock을 (origin_x, origin_y)부터 캔버스에 찍습니다.
/// 각 글자의 도장(coverage)을 색과 합성해 그립니다 — 진하기가 곧 알파.
pub fn paint_text(
    canvas: &mut Canvas,
    block: &TextBlock,
    origin_x: f32,
    origin_y: f32,
    font: &dyn FontFace,
    font_size: f32,
    color: Color,
) {
    if color.a == 0 {
        return;
    }
    let ascent = font.ascent(font_size);

    for line in &block.lines {
        // 이 줄의 기준선(baseline). 글자는 기준선을 기준으로 위로 올라갑니다.
        let baseline = origin_y + line.top + ascent;
        for g in &line.glyphs {
            let gb = font.rasterize(g.ch, font_size);
            if gb.width == 0 || gb.height == 0 {
                continue; // 공백 등 빈 글자.
            }
            // 도장의 왼쪽 위 픽셀 위치.
            let dst_x = (origin_x + g.x + gb.left).round() as i32;
            let dst_y = (baseline - gb.top).round() as i32;

            for gy in 0..gb.height {
                for gx in 0..gb.width {
                    let cov = gb.coverage[gy * gb.width + gx];
                    if cov == 0 {
                        continue;
                    }
                    // 진하기(0~255) x 글자색 알파 = 이 픽셀의 최종 알파.
                    let a = (cov as u16 * color.a as u16 / 255) as u8;
                    canvas.blend_pixel(
                        dst_x + gx as i32,
                        dst_y + gy as i32,
                        Color::rgba(color.r, color.g, color.b, a),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::BitmapFont;

    #[test]
    fn wraps_into_multiple_lines() {
        // 좁은 폭에 여러 단어 → 한 줄에 다 못 들어가 여러 줄이 되어야 함.
        let block = layout_text(
            "one two three four five",
            &BitmapFont,
            14.0,
            60.0,
            TextAlign::Left,
        );
        assert!(block.lines.len() > 1, "줄나눔이 일어나지 않았습니다");
        assert!(block.height >= block.lines.len() as f32);
    }

    #[test]
    fn center_shifts_glyphs_right() {
        // 가운데 정렬이면 첫 글자가 0보다 오른쪽에서 시작해야 함(여백이 있을 때).
        let left = layout_text("hi", &BitmapFont, 14.0, 200.0, TextAlign::Left);
        let center = layout_text("hi", &BitmapFont, 14.0, 200.0, TextAlign::Center);
        assert_eq!(left.lines[0].glyphs[0].x, 0.0);
        assert!(center.lines[0].glyphs[0].x > 0.0);
    }
}
