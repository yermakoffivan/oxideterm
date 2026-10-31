//! Kitty's Unicode placeholder data, decoded without expanding compact history rows.

use crate::grid::Row;
use crate::term::cell::Cell;
use crate::vte::ansi::{Color, NamedColor};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceholderRun {
    pub col: usize,
    pub len: usize,
    pub image: u32,
    pub placement: u32,
    pub image_row: usize,
    pub image_col: usize,
}

fn color_id(color: Color) -> u32 {
    match color {
        Color::Spec(rgb) => (u32::from(rgb.r) << 16) | (u32::from(rgb.g) << 8) | u32::from(rgb.b),
        Color::Indexed(index) => u32::from(index),
        Color::Named(color) if (color as u32) < 256 => color as u32,
        _ => 0,
    }
}

pub(super) fn decode_row(row: &Row<Cell>) -> Vec<PlaceholderRun> {
    let mut runs: Vec<PlaceholderRun> = Vec::new();
    let mut previous: Option<(Color, Option<Color>, usize, usize, u32)> = None;
    for (col, cell) in row.iter().enumerate() {
        if cell.c != '\u{10eeee}' {
            previous = None;
            continue;
        }
        let underline = cell.underline_color();
        let mut marks = cell
            .zerowidth()
            .unwrap_or_default()
            .iter()
            .filter_map(|c| DIACRITICS.binary_search(&(*c as u32)).ok());
        let row_mark = marks.next();
        let col_mark = marks.next();
        let high_mark = marks.next();
        let same = previous.filter(|(fg, ul, _, _, _)| *fg == cell.fg && *ul == underline);
        let image_row = row_mark.unwrap_or_else(|| same.map_or(0, |p| p.2));
        let continuation = same.filter(|p| p.2 == image_row);
        let image_col = col_mark.unwrap_or_else(|| continuation.map_or(0, |p| p.3 + 1));
        let high = high_mark.map(|h| h as u32).unwrap_or_else(|| {
            continuation
                .filter(|p| p.3 + 1 == image_col)
                .map_or(0, |p| p.4)
        });
        if high > 255 {
            previous = None;
            continue;
        }
        let image = color_id(cell.fg) | (high << 24);
        let placement = color_id(underline.unwrap_or(Color::Named(NamedColor::Foreground)));
        previous = Some((cell.fg, underline, image_row, image_col, high));
        if image == 0 {
            continue;
        }
        if let Some(last) = runs.last_mut()
            && last.col + last.len == col
            && last.image == image
            && last.placement == placement
            && last.image_row == image_row
            && last.image_col + last.len == image_col
        {
            last.len += 1;
        } else {
            runs.push(PlaceholderRun {
                col,
                len: 1,
                image,
                placement,
                image_row,
                image_col,
            });
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Column;

    #[test]
    fn placeholder_runs_inherit_only_from_contiguous_matching_cells() {
        let mut row = Row::<Cell>::new(8);
        for col in [0, 1, 2, 4, 5, 6, 7] {
            let cell = row.cell_mut(Column(col));
            cell.c = '\u{10eeee}';
            cell.fg = Color::Indexed(42);
            cell.set_underline_color(Some(Color::Indexed(7)));
        }
        for mark in ['\u{30d}', '\u{305}', '\u{30e}'] {
            row.cell_mut(Column(0)).push_zerowidth(mark);
        }
        row.cell_mut(Column(2)).push_zerowidth('\u{30d}');
        row.cell_mut(Column(5)).fg = Color::Spec(crate::vte::ansi::Rgb { r: 0, g: 0, b: 42 });
        row.cell_mut(Column(6)).push_zerowidth('\u{305}');
        row.cell_mut(Column(6)).push_zerowidth('\u{310}');
        for mark in ['\u{305}', '\u{312}'] {
            row.cell_mut(Column(7)).push_zerowidth(mark);
        }
        assert_eq!(
            decode_row(&row),
            [
                PlaceholderRun {
                    col: 0,
                    len: 3,
                    image: 33554474,
                    placement: 7,
                    image_row: 1,
                    image_col: 0
                },
                PlaceholderRun {
                    col: 4,
                    len: 1,
                    image: 42,
                    placement: 7,
                    image_row: 0,
                    image_col: 0
                },
                PlaceholderRun {
                    col: 5,
                    len: 1,
                    image: 42,
                    placement: 7,
                    image_row: 0,
                    image_col: 0
                },
                PlaceholderRun {
                    col: 6,
                    len: 2,
                    image: 42,
                    placement: 7,
                    image_row: 0,
                    image_col: 3
                },
            ]
        );
    }
}

// Normative codepoint order from kitty v0.49.2 gen/rowcolumn-diacritics.txt,
// derived from Unicode 6.0 data. These values are protocol data, not renderer code.
const DIACRITICS: &[u32] = &[
    0x0305, 0x030d, 0x030e, 0x0310, 0x0312, 0x033d, 0x033e, 0x033f, 0x0346, 0x034a, 0x034b, 0x034c,
    0x0350, 0x0351, 0x0352, 0x0357, 0x035b, 0x0363, 0x0364, 0x0365, 0x0366, 0x0367, 0x0368, 0x0369,
    0x036a, 0x036b, 0x036c, 0x036d, 0x036e, 0x036f, 0x0483, 0x0484, 0x0485, 0x0486, 0x0487, 0x0592,
    0x0593, 0x0594, 0x0595, 0x0597, 0x0598, 0x0599, 0x059c, 0x059d, 0x059e, 0x059f, 0x05a0, 0x05a1,
    0x05a8, 0x05a9, 0x05ab, 0x05ac, 0x05af, 0x05c4, 0x0610, 0x0611, 0x0612, 0x0613, 0x0614, 0x0615,
    0x0616, 0x0617, 0x0657, 0x0658, 0x0659, 0x065a, 0x065b, 0x065d, 0x065e, 0x06d6, 0x06d7, 0x06d8,
    0x06d9, 0x06da, 0x06db, 0x06dc, 0x06df, 0x06e0, 0x06e1, 0x06e2, 0x06e4, 0x06e7, 0x06e8, 0x06eb,
    0x06ec, 0x0730, 0x0732, 0x0733, 0x0735, 0x0736, 0x073a, 0x073d, 0x073f, 0x0740, 0x0741, 0x0743,
    0x0745, 0x0747, 0x0749, 0x074a, 0x07eb, 0x07ec, 0x07ed, 0x07ee, 0x07ef, 0x07f0, 0x07f1, 0x07f3,
    0x0816, 0x0817, 0x0818, 0x0819, 0x081b, 0x081c, 0x081d, 0x081e, 0x081f, 0x0820, 0x0821, 0x0822,
    0x0823, 0x0825, 0x0826, 0x0827, 0x0829, 0x082a, 0x082b, 0x082c, 0x082d, 0x0951, 0x0953, 0x0954,
    0x0f82, 0x0f83, 0x0f86, 0x0f87, 0x135d, 0x135e, 0x135f, 0x17dd, 0x193a, 0x1a17, 0x1a75, 0x1a76,
    0x1a77, 0x1a78, 0x1a79, 0x1a7a, 0x1a7b, 0x1a7c, 0x1b6b, 0x1b6d, 0x1b6e, 0x1b6f, 0x1b70, 0x1b71,
    0x1b72, 0x1b73, 0x1cd0, 0x1cd1, 0x1cd2, 0x1cda, 0x1cdb, 0x1ce0, 0x1dc0, 0x1dc1, 0x1dc3, 0x1dc4,
    0x1dc5, 0x1dc6, 0x1dc7, 0x1dc8, 0x1dc9, 0x1dcb, 0x1dcc, 0x1dd1, 0x1dd2, 0x1dd3, 0x1dd4, 0x1dd5,
    0x1dd6, 0x1dd7, 0x1dd8, 0x1dd9, 0x1dda, 0x1ddb, 0x1ddc, 0x1ddd, 0x1dde, 0x1ddf, 0x1de0, 0x1de1,
    0x1de2, 0x1de3, 0x1de4, 0x1de5, 0x1de6, 0x1dfe, 0x20d0, 0x20d1, 0x20d4, 0x20d5, 0x20d6, 0x20d7,
    0x20db, 0x20dc, 0x20e1, 0x20e7, 0x20e9, 0x20f0, 0x2cef, 0x2cf0, 0x2cf1, 0x2de0, 0x2de1, 0x2de2,
    0x2de3, 0x2de4, 0x2de5, 0x2de6, 0x2de7, 0x2de8, 0x2de9, 0x2dea, 0x2deb, 0x2dec, 0x2ded, 0x2dee,
    0x2def, 0x2df0, 0x2df1, 0x2df2, 0x2df3, 0x2df4, 0x2df5, 0x2df6, 0x2df7, 0x2df8, 0x2df9, 0x2dfa,
    0x2dfb, 0x2dfc, 0x2dfd, 0x2dfe, 0x2dff, 0xa66f, 0xa67c, 0xa67d, 0xa6f0, 0xa6f1, 0xa8e0, 0xa8e1,
    0xa8e2, 0xa8e3, 0xa8e4, 0xa8e5, 0xa8e6, 0xa8e7, 0xa8e8, 0xa8e9, 0xa8ea, 0xa8eb, 0xa8ec, 0xa8ed,
    0xa8ee, 0xa8ef, 0xa8f0, 0xa8f1, 0xaab0, 0xaab2, 0xaab3, 0xaab7, 0xaab8, 0xaabe, 0xaabf, 0xaac1,
    0xfe20, 0xfe21, 0xfe22, 0xfe23, 0xfe24, 0xfe25, 0xfe26, 0x10a0f, 0x10a38, 0x1d185, 0x1d186,
    0x1d187, 0x1d188, 0x1d189, 0x1d1aa, 0x1d1ab, 0x1d1ac, 0x1d1ad, 0x1d242, 0x1d243, 0x1d244,
];
