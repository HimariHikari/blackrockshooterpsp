//! Textures and the stand-in material used until the game's render state is decoded.
//!
//! The PSP works on the numbers its textures and colours hold, from start to end:
//! it multiplies them, mixes them with the fog's and with what is on the screen, and
//! shows what comes of it. A graphics card of today would have them be light first,
//! which mixes otherwise: a fog a fifth of the way in lifts a dark wall half as far
//! again. So the textures are given to it as plain numbers, the colours of the
//! meshes and of the fog likewise ([`values`]), and the cameras of the scenes mix
//! and show them as such ([`camera_of_values`]).

use bevy::asset::RenderAssetUsages;
use bevy::camera::CompositingSpace;
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use brs_formats::ptmd;

/// The rows a texture has for the PSP (0x250EC, where one is given to be drawn
/// from): as many as the first power of two its own rows fit in. Where something
/// is drawn from in a texture is told by that height, not by the rows it has.
pub fn rows(header: &ptmd::Header) -> usize {
    header.height.next_power_of_two()
}

/// The size of the image that [`image`] makes of a texture.
pub fn size(header: &ptmd::Header) -> Vec2 {
    Vec2::new(header.width as f32, rows(header) as f32)
}

/// Decodes the texture at the start of `data` into a repeating image, of the size
/// the PSP takes the texture to be: the rows past its own have nothing in them.
pub fn image(data: &[u8], header: &ptmd::Header) -> Image {
    let mut pixels = ptmd::decode(data, header);
    pixels.resize(header.width * rows(header) * 4, 0);
    image_of(pixels, header.width, rows(header))
}

/// An image of pixels that are decoded already (red, green, blue and alpha, a byte
/// each), kept and drawn from as [`image`] has a texture's.
pub fn image_of(pixels: Vec<u8>, width: usize, height: usize) -> Image {
    let size = Extent3d { width: width as u32, height: height as u32, depth_or_array_layers: 1 };
    let mut image = Image::new(
        size,
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..default()
    });
    image
}

/// The size of the original's screen, in its pixels.
pub const SCREEN_WIDTH: usize = 480;
pub const SCREEN_HEIGHT: usize = 272;

// How a picture that fills the screen is kept in a texture of 512 by 256 (0x2DE94):
// the screen's first 256 rows are the texture's left 480 columns, and its last 16
// rows lie on their side in the 32 columns left over, as two strips of 16 by 240
// that are drawn turned a quarter turn, one for each half of the screen's width.
const PICTURE_WIDTH: usize = 512;
const PICTURE_HEIGHT: usize = 256;
const STRIP_WIDTH: usize = 16;
const STRIP_LENGTH: usize = 240;

/// The pixels of a picture that fills the screen, from those of its texture; nothing
/// if the texture is not of the size such pictures are kept in.
pub fn screen_pixels(texture: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    if width != PICTURE_WIDTH || height != PICTURE_HEIGHT || texture.len() < width * height * 4 {
        return None;
    }
    let mut screen = Vec::with_capacity(SCREEN_WIDTH * SCREEN_HEIGHT * 4);
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            let (u, v) = if y < PICTURE_HEIGHT {
                (x, y)
            } else {
                // The strip's first column is the screen's last row.
                let (strip, along) = (x / STRIP_LENGTH, x % STRIP_LENGTH);
                (SCREEN_WIDTH + strip * STRIP_WIDTH + (STRIP_WIDTH - 1 - (y - PICTURE_HEIGHT)), along)
            };
            let at = (v * width + u) * 4;
            screen.extend_from_slice(&texture[at..at + 4]);
        }
    }
    Some(screen)
}

/// Decodes the texture at the start of `data` as a picture that fills the screen.
pub fn screen_picture(data: &[u8], header: &ptmd::Header) -> Option<Image> {
    let pixels = screen_pixels(&ptmd::decode(data, header), header.width, header.height)?;
    let size = Extent3d { width: SCREEN_WIDTH as u32, height: SCREEN_HEIGHT as u32, depth_or_array_layers: 1 };
    Some(Image::new(size, TextureDimension::D2, pixels, TextureFormat::Rgba8Unorm, RenderAssetUsages::default()))
}

/// A colour as a material or a fog is to be given it, so that what draws with it
/// has the colour's own numbers and not the light they stand for.
pub fn values(colour: Color) -> Color {
    let Srgba { red, green, blue, alpha } = colour.to_srgba();
    Color::linear_rgba(red, green, blue, alpha)
}

/// What a camera of a scene has for its picture to be made of the game's own
/// numbers: they are mixed as they are, and neither mapped to a film's look nor
/// dithered.
pub fn camera_of_values() -> impl Bundle {
    (CompositingSpace::Srgb, Tonemapping::None, DebandDither::Disabled)
}

/// Whether any pixel of `image` is partly see-through, rather than fully there or fully
/// cut away.
pub fn has_soft_alpha(image: &Image) -> bool {
    image.data.as_ref().is_some_and(|data| data.chunks_exact(4).any(|pixel| pixel[3] != 0 && pixel[3] != 255))
}

/// The lighting and blend setup of models is not decoded yet: draw unlit, alpha-tested
/// and double-sided so nothing is hidden by a wrong guess.
pub fn flat_material(texture: Option<Handle<Image>>) -> StandardMaterial {
    StandardMaterial {
        base_color_texture: texture,
        unlit: true,
        alpha_mode: AlphaMode::Mask(0.5),
        cull_mode: None,
        double_sided: true,
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_picture_takes_its_last_rows_from_the_strips_on_their_side() {
        // Each pixel of the texture is marked with where it is.
        let mut texture = Vec::new();
        for v in 0..PICTURE_HEIGHT {
            for u in 0..PICTURE_WIDTH {
                texture.extend([(u % 256) as u8, (u / 256) as u8, v as u8, 255]);
            }
        }
        let screen = screen_pixels(&texture, PICTURE_WIDTH, PICTURE_HEIGHT).unwrap();
        assert_eq!(screen.len(), SCREEN_WIDTH * SCREEN_HEIGHT * 4);
        let from = |x: usize, y: usize| {
            let pixel = &screen[(y * SCREEN_WIDTH + x) * 4..][..4];
            (pixel[0] as usize + pixel[1] as usize * 256, pixel[2] as usize)
        };
        // The top of the screen is the texture as it is.
        assert_eq!(from(0, 0), (0, 0));
        assert_eq!(from(479, 255), (479, 255));
        // Row 256 is the last column of each strip, row 271 the first; along the
        // row the strip is read downwards, the second strip for the right half.
        assert_eq!(from(0, 256), (495, 0));
        assert_eq!(from(239, 256), (495, 239));
        assert_eq!(from(240, 256), (511, 0));
        assert_eq!(from(0, 271), (480, 0));
        assert_eq!(from(479, 271), (496, 239));
        // A texture of another size is no such picture.
        assert!(screen_pixels(&texture, 256, 512).is_none());
    }
}