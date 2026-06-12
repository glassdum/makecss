//! 폰트의 공통 인터페이스(FontFace)와 글자 도장(GlyphBitmap).
//!
//! 핵심 아이디어: 레이아웃과 페인트는 '폰트가 비트맵이든 TTF든' 신경 쓰지 않습니다.
//! 모든 폰트는 아래 약속(FontFace)만 지키면 됩니다:
//!   - advance : 이 글자를 그린 뒤 다음 글자까지 옆으로 갈 거리(px)
//!   - rasterize : 이 글자의 '도장 자국'(픽셀별 진하기) 만들기
//!   - ascent / line_height : 글자를 줄에 앉히는 데 필요한 세로 기준
//!
//! 비유: 만능 도장틀. 글자만 끼우면 폭과 자국을 내주는 기계. 비트맵 폰트와
//! TTF 폰트는 '서로 다른 도장틀'이지만, 사용하는 쪽은 똑같이 다룹니다.

/// 글자 하나를 픽셀로 찍은 '도장 자국'.
/// 각 픽셀의 값은 진하기(coverage) 0~255. 0 = 안 칠함, 255 = 꽉 칠함.
/// (진하기를 알파로 써서 가장자리를 부드럽게 = 안티앨리어싱)
pub struct GlyphBitmap {
    pub width: usize,
    pub height: usize,
    /// 펜 위치에서 도장 자국 왼쪽 끝까지의 가로 거리(px). 보통 0 근처.
    pub left: f32,
    /// 기준선(baseline)에서 도장 자국 위쪽 끝까지의 거리(px). 위로 +.
    pub top: f32,
    /// 길이 = width * height. (gx, gy)는 gy*width + gx. 값은 진하기 0~255.
    pub coverage: Vec<u8>,
}

impl GlyphBitmap {
    /// 빈 자국(예: 공백 문자). 폭/높이 0.
    pub fn empty() -> Self {
        GlyphBitmap { width: 0, height: 0, left: 0.0, top: 0.0, coverage: Vec::new() }
    }
}

/// 모든 폰트가 지켜야 할 약속.
/// 레이아웃·페인트는 `&dyn FontFace`로 어떤 폰트든 똑같이 사용합니다.
pub trait FontFace {
    /// 기준선에서 글자 윗부분까지의 높이(px). 글자를 줄에 앉힐 때의 기준.
    fn ascent(&self, font_size: f32) -> f32;

    /// 한 줄이 차지하는 세로 높이(px). 다음 줄은 이만큼 아래로 내려갑니다.
    fn line_height(&self, font_size: f32) -> f32;

    /// 이 글자를 그린 뒤 다음 글자까지의 가로 거리(px).
    fn advance(&self, ch: char, font_size: f32) -> f32;

    /// 이 글자의 도장 자국(픽셀별 진하기)을 만듭니다.
    fn rasterize(&self, ch: char, font_size: f32) -> GlyphBitmap;
}
