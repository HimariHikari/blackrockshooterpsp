//! Drawing the sign that names a place (0x46EBC): each of its letters that is
//! there, a cell of its sheet where the letter is, white and as much there as
//! the letter is; and after them its star.

use anyhow::{Context, Result};
use bevy::prelude::*;
use brs_engine::canvas::{Canvas, Paint, Vertex};
use brs_engine::texture;
use brs_formats::ptmd;

use crate::data::GameData;
use crate::field::place::{Sign, GLYPH, SHEET};

/// The texels that are put round each cell of the sheet.
const EDGE: usize = 1;

/// The sheet of the sign's letters, as it is drawn from here. The original's has
/// its cells side by side with nothing between them, and a letter fills its cell
/// to the edge; the PSP draws them a texel to a pixel, where that does not
/// matter. Enlarged to a window, the pixels at a letter's edge are a mix of two
/// texels, and the second would be the edge of whatever letter is next to it in
/// the sheet. So each cell is given its own edge once more all round, and a
/// letter's edge is mixed with nothing but itself.
pub struct Sheet {
    image: Handle<Image>,
    size: Vec2,
}

impl Sheet {
    pub fn load(data: &GameData, images: &mut Assets<Image>) -> Result<Sheet> {
        let bytes = data.read(SHEET)?;
        let header = ptmd::parse_header(&bytes).with_context(|| format!("{SHEET} is not a texture"))?;
        let (pixels, cell) = (ptmd::decode(&bytes, &header), GLYPH as usize);
        let (columns, rows, wide) = (header.width / cell, header.height / cell, cell + 2 * EDGE);
        let (width, height) = (columns * wide, rows * wide);
        let mut padded = vec![0; width * height * 4];
        for y in 0..height {
            for x in 0..width {
                // The texel of the cell that is nearest to this one of its copy.
                let within = |at: usize| (at / wide) * cell + (at % wide).saturating_sub(EDGE).min(cell - 1);
                let (from, to) = ((within(y) * header.width + within(x)) * 4, (y * width + x) * 4);
                padded[to..to + 4].copy_from_slice(&pixels[from..from + 4]);
            }
        }
        Ok(Sheet { image: images.add(texture::image_of(padded, width, height)), size: Vec2::new(width as f32, height as f32) })
    }

    /// Where a cell of the original's sheet is in this one.
    fn place(&self, source: [i32; 2]) -> Vec2 {
        let wide = GLYPH + 2 * EDGE as i32;
        (IVec2::from(source) / GLYPH * wide + EDGE as i32).as_vec2()
    }
}

/// Draws a sign from the sheet of its letters (0x32AD4 for each).
pub fn draw(canvas: &mut Canvas, layer: i32, sheet: &Sheet, sign: &Sign) {
    let span = Vec2::splat(GLYPH as f32);
    for cell in sign.rows.iter().flatten().chain([&sign.cursor]).filter(|cell| cell.alpha != 0) {
        let (at, from) = (IVec2::from(cell.at).as_vec2(), sheet.place(cell.source));
        let tint = [0xff, 0xff, 0xff, cell.alpha];
        let paint = Paint::textured(&sheet.image);
        canvas.sprite(layer, paint, Vertex::new(at, from / sheet.size, tint), Vertex::new(at + span, (from + span) / sheet.size, tint));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::place::{PlaceName, Places, Title, STAR};

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked");

    #[test]
    fn a_sign_is_drawn_a_cell_of_its_sheet_for_each_letter_that_is_there() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut images = Assets::<Image>::default();
        let sheet = Sheet::load(&data, &mut images).unwrap();
        // Eleven cells across and four down, each with its edge round it.
        assert_eq!(sheet.size, Vec2::new(11.0 * 13.0, 4.0 * 13.0));
        assert_eq!((sheet.place([0, 0]), sheet.place([88, 11]), sheet.place(STAR)), (Vec2::new(1.0, 1.0), Vec2::new(105.0, 14.0), Vec2::new(105.0, 27.0)));
        let places = Places::load(&data).unwrap();
        let drawn = |sign: &Sign| {
            let mut canvas = Canvas::default();
            draw(&mut canvas, 0, &sheet, sign);
            canvas.drawn_from(&sheet.image)
        };

        // The name of a place is all there at once: its first letter, a T, is the
        // ninth of the sheet's second row, drawn eleven square at the foot of
        // the screen on the left.
        let mut shown = None;
        PlaceName::name(&mut shown, &places, 12, 90);
        let name = shown.unwrap();
        let all = drawn(&name.sign);
        let corner = |at: [f32; 2], from: Vec2| all.iter().any(|vertex| vertex.position == Vec2::from(at) && vertex.uv == from / sheet.size);
        let letter = sheet.place([88, 11]);
        assert!(corner([6.0, 217.0], letter) && corner([17.0, 228.0], letter + 11.0));
        assert!(all.iter().all(|vertex| vertex.colour == [0xff; 4]));
        // Nineteen letters, and no star.
        let a_sprite = {
            let mut canvas = Canvas::default();
            canvas.sprite(0, Paint::textured(&sheet.image), Vertex::new(Vec2::ZERO, Vec2::ZERO, [0xff; 4]), Vertex::new(Vec2::ONE, Vec2::ONE, [0xff; 4]));
            canvas.drawn_from(&sheet.image).len()
        };
        let star = sheet.place(STAR) / sheet.size;
        assert_eq!(all.len(), 19 * a_sprite);
        assert!(all.chunks(a_sprite).all(|sprite| sprite[0].uv != star));

        // Of a title that is being typed there is the star alone at first, and
        // then the letters typed so far.
        let mut title = Title::new(0);
        assert!(drawn(&title.sign).is_empty());
        title.start(&places, 60);
        title.step();
        let first = drawn(&title.sign);
        assert_eq!((first.len(), first[0].position, first[0].uv), (a_sprite, Vec2::new(6.0, 6.0), star));
        title.step();
        let second = drawn(&title.sign);
        // A "2": the third of the figures, in the sheet's fourth row.
        assert_eq!((second.len(), second[0].position, second[0].uv), (a_sprite, Vec2::new(6.0, 6.0), sheet.place([22, 33]) / sheet.size));
    }

    #[test]
    fn a_cell_of_the_sheet_has_its_own_edge_round_it() {
        let Ok(data) = GameData::new(DATA) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let mut images = Assets::<Image>::default();
        let sheet = Sheet::load(&data, &mut images).unwrap();
        let bytes = data.read(SHEET).unwrap();
        let header = ptmd::parse_header(&bytes).unwrap();
        let original = ptmd::decode(&bytes, &header);
        let padded = images.get(&sheet.image).unwrap().data.clone().unwrap();
        let of = |x: usize, y: usize| &original[(y * header.width + x) * 4..][..4];
        let copy = |x: usize, y: usize| &padded[(y * sheet.size.x as usize + x) * 4..][..4];
        // The second cell of the second row, an M: its texels are where the cell
        // is put, and those round it are its edge's again, not its neighbours'.
        let (cell, to) = ([11, 11], [14, 14]);
        for y in 0..11 {
            for x in 0..11 {
                assert_eq!(copy(to[0] + x, to[1] + y), of(cell[0] + x, cell[1] + y));
            }
            assert_eq!((copy(to[0] - 1, to[1] + y), copy(to[0] + 11, to[1] + y)), (of(cell[0], cell[1] + y), of(cell[0] + 10, cell[1] + y)));
        }
        for x in 0..11 {
            assert_eq!((copy(to[0] + x, to[1] - 1), copy(to[0] + x, to[1] + 11)), (of(cell[0] + x, cell[1]), of(cell[0] + x, cell[1] + 10)));
        }
        // Its neighbour to the right begins with something else, which is what
        // would have been mixed in.
        assert!((0..11).any(|y| of(cell[0] + 11, cell[1] + y) != of(cell[0] + 10, cell[1] + y)));
    }
}
