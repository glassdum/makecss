//! 진짜 TrueType(.ttf) 폰트 렌더러 — 의존성 0으로 직접 구현.
//!
//! 비트맵 폰트가 '이미 점이 찍힌 도장'이라면, TTF는 '테두리 좌표만 적힌 설계도'입니다.
//! 글자 'o'는 "이 점들을 이어 곡선을 그리면 원" 이라는 좌표 목록일 뿐입니다.
//!
//! 우리가 하는 일은 세 단계:
//!   1) 파싱   : 폰트 파일의 표(table)들을 읽어 글자별 외곽선 좌표를 얻는다.
//!   2) 평탄화 : 곡선(2차 베지에)을 잘게 쪼개 직선들의 다각형으로 편다.
//!   3) 채우기 : 다각형 '안쪽'에 드는 픽셀을 칠한다. 가장자리는 여러 점을 찍어
//!               평균(=커버리지)으로 부드럽게 = 안티앨리어싱(supersampling).
//!
//! 지원: glyf(TrueType 외곽선) + cmap 포맷 4(기본 다국어 평면). CFF(.otf 계열)는
//! 외곽선 저장 방식이 달라 지원하지 않습니다.

use crate::fontface::{FontFace, GlyphBitmap};

/// 외곽선 위의 한 점. on=true면 곡선이 지나는 점, false면 곡선을 당기는 제어점.
#[derive(Clone, Copy)]
struct Point {
    x: f32,
    y: f32,
    on: bool,
}

/// cmap 포맷 4(글자 → 글리프 번호 매핑)에서 뽑아둔 표.
struct Cmap {
    seg_end: Vec<u16>,
    seg_start: Vec<u16>,
    id_delta: Vec<i16>,
    id_range_offset: Vec<u16>,
    /// idRangeOffset 배열이 파일에서 시작하는 바이트 위치(간접 참조 계산용).
    id_range_offset_pos: usize,
}

/// 파싱이 끝난 TrueType 폰트.
pub struct TtfFont {
    data: Vec<u8>,
    units_per_em: f32,
    ascender: f32,
    descender: f32,
    line_gap: f32,
    num_glyphs: u16,
    loc_long: bool, // loca 오프셋이 4바이트(true)인지 2바이트(false)인지.
    loca_off: usize,
    glyf_off: usize,
    hmtx_off: usize,
    num_h_metrics: u16,
    cmap: Cmap,
}

// ── 빅엔디안 읽기 도우미 (범위를 벗어나면 0을 돌려 안전하게) ──
fn be_u16(d: &[u8], off: usize) -> u16 {
    if off + 2 > d.len() {
        return 0;
    }
    ((d[off] as u16) << 8) | d[off + 1] as u16
}
fn be_i16(d: &[u8], off: usize) -> i16 {
    be_u16(d, off) as i16
}
fn be_u32(d: &[u8], off: usize) -> u32 {
    if off + 4 > d.len() {
        return 0;
    }
    ((d[off] as u32) << 24)
        | ((d[off + 1] as u32) << 16)
        | ((d[off + 2] as u32) << 8)
        | d[off + 3] as u32
}
/// F2Dot14 고정소수점(합성 글리프 배율) → f32.
fn f2dot14(v: i16) -> f32 {
    v as f32 / 16384.0
}

impl TtfFont {
    /// .ttf 파일의 바이트들을 받아 파싱합니다.
    pub fn from_bytes(data: Vec<u8>) -> Result<TtfFont, String> {
        // ── 표 목록(table directory) 읽기 ──
        let num_tables = be_u16(&data, 4) as usize;
        let find = |tag: &[u8; 4]| -> Option<(usize, usize)> {
            for i in 0..num_tables {
                let rec = 12 + i * 16;
                if rec + 16 > data.len() {
                    return None;
                }
                if &data[rec..rec + 4] == tag {
                    let off = be_u32(&data, rec + 8) as usize;
                    let len = be_u32(&data, rec + 12) as usize;
                    return Some((off, len));
                }
            }
            None
        };

        let head = find(b"head").ok_or("head 표 없음")?;
        let units_per_em = be_u16(&data, head.0 + 18) as f32;
        let loc_long = be_i16(&data, head.0 + 50) != 0;

        let maxp = find(b"maxp").ok_or("maxp 표 없음")?;
        let num_glyphs = be_u16(&data, maxp.0 + 4);

        let hhea = find(b"hhea").ok_or("hhea 표 없음")?;
        let ascender = be_i16(&data, hhea.0 + 4) as f32;
        let descender = be_i16(&data, hhea.0 + 6) as f32;
        let line_gap = be_i16(&data, hhea.0 + 8) as f32;
        let num_h_metrics = be_u16(&data, hhea.0 + 34);

        let hmtx = find(b"hmtx").ok_or("hmtx 표 없음")?;
        let loca = find(b"loca").ok_or("loca 표 없음")?;
        let glyf = find(b"glyf").ok_or("glyf 표 없음(CFF/.otf 폰트는 미지원)")?;
        let cmap_tab = find(b"cmap").ok_or("cmap 표 없음")?;

        let cmap = parse_cmap(&data, cmap_tab.0)?;

        Ok(TtfFont {
            data,
            units_per_em: if units_per_em == 0.0 { 1000.0 } else { units_per_em },
            ascender,
            descender,
            line_gap,
            num_glyphs,
            loc_long,
            loca_off: loca.0,
            glyf_off: glyf.0,
            hmtx_off: hmtx.0,
            num_h_metrics,
            cmap,
        })
    }

    /// 글자 → 글리프 번호(없으면 0 = .notdef).
    fn glyph_index(&self, ch: char) -> u16 {
        let c = ch as u32;
        if c > 0xffff {
            return 0; // 포맷 4는 기본 다국어 평면(0~0xFFFF)만.
        }
        let c = c as u16;
        let cm = &self.cmap;
        for i in 0..cm.seg_end.len() {
            if cm.seg_end[i] >= c {
                if cm.seg_start[i] > c {
                    return 0;
                }
                if cm.id_range_offset[i] == 0 {
                    return (c as i32 + cm.id_delta[i] as i32) as u16;
                }
                // 간접 참조: glyphIdArray에서 실제 번호를 찾습니다.
                let addr = cm.id_range_offset_pos
                    + i * 2
                    + cm.id_range_offset[i] as usize
                    + (c - cm.seg_start[i]) as usize * 2;
                let g = be_u16(&self.data, addr);
                if g == 0 {
                    return 0;
                }
                return (g as i32 + cm.id_delta[i] as i32) as u16;
            }
        }
        0
    }

    /// 글리프의 진행폭(advance width, 폰트 단위).
    fn advance_width(&self, gid: u16) -> u16 {
        let i = if gid < self.num_h_metrics {
            gid as usize
        } else {
            (self.num_h_metrics.max(1) - 1) as usize
        };
        be_u16(&self.data, self.hmtx_off + i * 4)
    }

    /// 글리프의 외곽선(폰트 단위 좌표). 합성 글리프는 부품을 합쳐 돌려줍니다.
    fn glyph_contours(&self, gid: u16, depth: u8) -> Vec<Vec<Point>> {
        if depth > 5 || gid >= self.num_glyphs {
            return Vec::new();
        }

        // loca 표에서 이 글리프의 데이터 위치(start..end)를 찾습니다.
        let (start, end) = if self.loc_long {
            (
                be_u32(&self.data, self.loca_off + gid as usize * 4) as usize,
                be_u32(&self.data, self.loca_off + (gid as usize + 1) * 4) as usize,
            )
        } else {
            (
                be_u16(&self.data, self.loca_off + gid as usize * 2) as usize * 2,
                be_u16(&self.data, self.loca_off + (gid as usize + 1) * 2) as usize * 2,
            )
        };
        if start >= end {
            return Vec::new(); // 빈 글리프(예: 공백).
        }

        let g = self.glyf_off + start;
        let num_contours = be_i16(&self.data, g);
        if num_contours < 0 {
            return self.parse_composite(g + 10, depth);
        }
        self.parse_simple(g, num_contours as usize)
    }

    /// 단순 글리프(직접 외곽선) 파싱.
    fn parse_simple(&self, g: usize, num_contours: usize) -> Vec<Vec<Point>> {
        let d = &self.data;
        // glyf 헤더(번호 + 경계상자) 10바이트 뒤부터 endPtsOfContours.
        let mut p = g + 10;
        let mut end_pts = Vec::with_capacity(num_contours);
        for _ in 0..num_contours {
            end_pts.push(be_u16(d, p));
            p += 2;
        }
        let num_points = match end_pts.last() {
            Some(&last) => last as usize + 1,
            None => return Vec::new(),
        };

        // 힌팅 명령어는 건너뜁니다.
        let instr_len = be_u16(d, p) as usize;
        p += 2 + instr_len;

        // ── 플래그(점마다 1바이트, 반복 압축 있음) ──
        let mut flags = Vec::with_capacity(num_points);
        while flags.len() < num_points && p < d.len() {
            let f = d[p];
            p += 1;
            flags.push(f);
            if f & 0x08 != 0 {
                // 다음 바이트만큼 같은 플래그 반복.
                let mut rep = d.get(p).copied().unwrap_or(0);
                p += 1;
                while rep > 0 && flags.len() < num_points {
                    flags.push(f);
                    rep -= 1;
                }
            }
        }
        flags.resize(num_points, 0);

        // ── x 좌표(델타 누적) ──
        let mut xs = Vec::with_capacity(num_points);
        let mut x = 0i32;
        for &f in &flags {
            if f & 0x02 != 0 {
                let dx = d.get(p).copied().unwrap_or(0) as i32;
                p += 1;
                x += if f & 0x10 != 0 { dx } else { -dx };
            } else if f & 0x10 == 0 {
                x += be_i16(d, p) as i32;
                p += 2;
            } // (0x10 set & 0x02 clear) → x 변화 없음.
            xs.push(x);
        }

        // ── y 좌표(델타 누적) ──
        let mut ys = Vec::with_capacity(num_points);
        let mut y = 0i32;
        for &f in &flags {
            if f & 0x04 != 0 {
                let dy = d.get(p).copied().unwrap_or(0) as i32;
                p += 1;
                y += if f & 0x20 != 0 { dy } else { -dy };
            } else if f & 0x20 == 0 {
                y += be_i16(d, p) as i32;
                p += 2;
            }
            ys.push(y);
        }

        // ── 점들을 윤곽선(contour)별로 묶습니다 ──
        let mut contours = Vec::with_capacity(num_contours);
        let mut s = 0usize;
        for &e in &end_pts {
            let e = e as usize;
            let mut contour = Vec::new();
            for i in s..=e.min(num_points - 1) {
                contour.push(Point {
                    x: xs[i] as f32,
                    y: ys[i] as f32,
                    on: flags[i] & 0x01 != 0,
                });
            }
            if !contour.is_empty() {
                contours.push(contour);
            }
            s = e + 1;
        }
        contours
    }

    /// 합성 글리프(다른 글리프들을 옮겨/늘려 조합) 파싱.
    fn parse_composite(&self, mut p: usize, depth: u8) -> Vec<Vec<Point>> {
        let d = &self.data;
        let mut out = Vec::new();
        loop {
            let flags = be_u16(d, p);
            let comp_gid = be_u16(d, p + 2);
            p += 4;

            // 이동값(보통 x,y 픽셀 오프셋).
            let (dx, dy);
            if flags & 0x0001 != 0 {
                dx = be_i16(d, p) as f32;
                dy = be_i16(d, p + 2) as f32;
                p += 4;
            } else {
                dx = (d.get(p).copied().unwrap_or(0) as i8) as f32;
                dy = (d.get(p + 1).copied().unwrap_or(0) as i8) as f32;
                p += 2;
            }

            // 2x2 변형(배율/회전).
            let (mut a, mut b, mut c, mut e) = (1.0f32, 0.0f32, 0.0f32, 1.0f32);
            if flags & 0x0008 != 0 {
                a = f2dot14(be_i16(d, p));
                e = a;
                p += 2;
            } else if flags & 0x0040 != 0 {
                a = f2dot14(be_i16(d, p));
                e = f2dot14(be_i16(d, p + 2));
                p += 4;
            } else if flags & 0x0080 != 0 {
                a = f2dot14(be_i16(d, p));
                b = f2dot14(be_i16(d, p + 2));
                c = f2dot14(be_i16(d, p + 4));
                e = f2dot14(be_i16(d, p + 6));
                p += 8;
            }

            // 부품 글리프를 가져와 변형을 적용해 합칩니다.
            for contour in self.glyph_contours(comp_gid, depth + 1) {
                let moved = contour
                    .iter()
                    .map(|pt| Point {
                        x: a * pt.x + c * pt.y + dx,
                        y: b * pt.x + e * pt.y + dy,
                        on: pt.on,
                    })
                    .collect();
                out.push(moved);
            }

            if flags & 0x0020 == 0 {
                break; // 더 이상 부품 없음.
            }
        }
        out
    }
}

/// cmap 표에서 포맷 4 유니코드 서브테이블을 찾아 파싱합니다.
fn parse_cmap(d: &[u8], cmap_off: usize) -> Result<Cmap, String> {
    let num_subtables = be_u16(d, cmap_off + 2) as usize;
    let mut best: Option<usize> = None;
    for i in 0..num_subtables {
        let rec = cmap_off + 4 + i * 8;
        let platform = be_u16(d, rec);
        let encoding = be_u16(d, rec + 2);
        let suboff = cmap_off + be_u32(d, rec + 4) as usize;
        let format = be_u16(d, suboff);
        if format == 4 {
            // 윈도우 BMP(3,1) 또는 유니코드(0,*)를 우선 선택.
            let preferred = (platform == 3 && encoding == 1) || platform == 0;
            if preferred {
                best = Some(suboff);
                break;
            }
            best.get_or_insert(suboff);
        }
    }
    let suboff = best.ok_or("cmap에 지원하는 포맷 4 서브테이블이 없음")?;

    let seg_x2 = be_u16(d, suboff + 6) as usize;
    let seg_count = seg_x2 / 2;
    let end_base = suboff + 14;
    let start_base = end_base + seg_x2 + 2; // +2 = reservedPad
    let delta_base = start_base + seg_x2;
    let range_base = delta_base + seg_x2;

    let read_arr_u16 = |base: usize| -> Vec<u16> {
        (0..seg_count).map(|i| be_u16(d, base + i * 2)).collect()
    };
    let read_arr_i16 = |base: usize| -> Vec<i16> {
        (0..seg_count).map(|i| be_i16(d, base + i * 2)).collect()
    };

    Ok(Cmap {
        seg_end: read_arr_u16(end_base),
        seg_start: read_arr_u16(start_base),
        id_delta: read_arr_i16(delta_base),
        id_range_offset: read_arr_u16(range_base),
        id_range_offset_pos: range_base,
    })
}

/// 윤곽선(on/off 점들)을 직선들의 다각형으로 폅니다. 좌표는 이미 화면 단위.
/// 2차 베지에(off 점)는 잘게 쪼개 직선으로 근사합니다.
fn flatten_contour(points: &[Point]) -> Vec<(f32, f32)> {
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }

    // 1) 연속한 두 제어점(off) 사이에는 '암묵적 on 점'(중점)이 있습니다 → 끼워넣기.
    let mut pts: Vec<Point> = Vec::with_capacity(n * 2);
    for i in 0..n {
        let cur = points[i];
        pts.push(cur);
        let next = points[(i + 1) % n];
        if !cur.on && !next.on {
            pts.push(Point {
                x: (cur.x + next.x) / 2.0,
                y: (cur.y + next.y) / 2.0,
                on: true,
            });
        }
    }
    // 2) 시작점이 on이 되도록 회전.
    let start_idx = pts.iter().position(|p| p.on).unwrap_or(0);
    let m = pts.len();
    let mut seq: Vec<Point> = (0..m).map(|k| pts[(start_idx + k) % m]).collect();
    seq.push(seq[0]); // 닫는 점(시작점) 추가.

    // 3) on-on은 직선, on-off-on은 2차 베지에로 그립니다.
    let mut out: Vec<(f32, f32)> = vec![(seq[0].x, seq[0].y)];
    let mut start = seq[0];
    let mut i = 1;
    while i < seq.len() {
        let cur = seq[i];
        if cur.on {
            out.push((cur.x, cur.y));
            start = cur;
            i += 1;
        } else {
            let ctrl = cur;
            let end = seq[i + 1]; // 마지막은 on(닫는 점)이라 항상 존재.
            flatten_quad(start, ctrl, end, &mut out);
            start = end;
            i += 2;
        }
    }
    out
}

/// 2차 베지에 곡선(시작 a, 제어 ctrl, 끝 b)을 짧은 직선 여러 개로 폅니다.
fn flatten_quad(a: Point, ctrl: Point, b: Point, out: &mut Vec<(f32, f32)>) {
    const STEPS: usize = 8;
    for s in 1..=STEPS {
        let t = s as f32 / STEPS as f32;
        let mt = 1.0 - t;
        // B(t) = (1-t)^2 a + 2(1-t)t ctrl + t^2 b
        let x = mt * mt * a.x + 2.0 * mt * t * ctrl.x + t * t * b.x;
        let y = mt * mt * a.y + 2.0 * mt * t * ctrl.y + t * t * b.y;
        out.push((x, y));
    }
}

/// 점 (px, py)가 다각형들(짝수-홀수 규칙) 안에 있는지 판정. 고전적인 광선 교차법.
fn inside(polys: &[Vec<(f32, f32)>], px: f32, py: f32) -> bool {
    let mut c = false;
    for poly in polys {
        let n = poly.len();
        if n < 3 {
            continue;
        }
        let mut j = n - 1;
        for i in 0..n {
            let (xi, yi) = poly[i];
            let (xj, yj) = poly[j];
            // 오른쪽으로 쏜 수평 광선이 변 (j→i)를 지나는가?
            if (yi > py) != (yj > py) {
                let x_cross = (xj - xi) * (py - yi) / (yj - yi) + xi;
                if px < x_cross {
                    c = !c;
                }
            }
            j = i;
        }
    }
    c
}

impl FontFace for TtfFont {
    fn ascent(&self, font_size: f32) -> f32 {
        self.ascender * font_size / self.units_per_em
    }

    fn line_height(&self, font_size: f32) -> f32 {
        (self.ascender - self.descender + self.line_gap) * font_size / self.units_per_em
    }

    fn advance(&self, ch: char, font_size: f32) -> f32 {
        let gid = self.glyph_index(ch);
        self.advance_width(gid) as f32 * font_size / self.units_per_em
    }

    fn rasterize(&self, ch: char, font_size: f32) -> GlyphBitmap {
        let gid = self.glyph_index(ch);
        let contours = self.glyph_contours(gid, 0);
        if contours.is_empty() {
            return GlyphBitmap::empty();
        }

        // 폰트 단위 → 화면 단위. y는 위가 +이므로 화면(아래가 +)에 맞게 뒤집습니다.
        let scale = font_size / self.units_per_em;
        let polys: Vec<Vec<(f32, f32)>> = contours
            .iter()
            .map(|c| {
                let screen: Vec<Point> = c
                    .iter()
                    .map(|p| Point { x: p.x * scale, y: -p.y * scale, on: p.on })
                    .collect();
                flatten_contour(&screen)
            })
            .collect();

        // 경계 상자.
        let (mut min_x, mut min_y, mut max_x, mut max_y) =
            (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for poly in &polys {
            for &(x, y) in poly {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
        let min_x = min_x.floor();
        let min_y = min_y.floor();
        let width = (max_x.ceil() - min_x).max(0.0) as usize;
        let height = (max_y.ceil() - min_y).max(0.0) as usize;
        if width == 0 || height == 0 {
            return GlyphBitmap::empty();
        }

        // 픽셀마다 SSxSS개의 점을 찍어 '안쪽'에 든 비율로 진하기를 정합니다(안티앨리어싱).
        const SS: usize = 4;
        let mut coverage = vec![0u8; width * height];
        for gy in 0..height {
            for gx in 0..width {
                let mut hits = 0u32;
                for sy in 0..SS {
                    for sx in 0..SS {
                        let px = min_x + gx as f32 + (sx as f32 + 0.5) / SS as f32;
                        let py = min_y + gy as f32 + (sy as f32 + 0.5) / SS as f32;
                        if inside(&polys, px, py) {
                            hits += 1;
                        }
                    }
                }
                coverage[gy * width + gx] = (hits * 255 / (SS * SS) as u32) as u8;
            }
        }

        GlyphBitmap {
            width,
            height,
            left: min_x,
            top: -min_y, // 경계 상자 윗변의 화면 y는 min_y(음수) → 기준선 위 거리 = -min_y.
            coverage,
        }
    }
}
