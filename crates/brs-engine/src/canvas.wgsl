// What the canvas is drawn with: a texture's colour by the corner's, as the PSP
// draws its interface. What has no texture is given a white one.
//
// The colours are the game's own values from start to end: a corner's come as they
// are, a texture's are kept as they are, and what is given out is mixed with what
// is there as it stands (the canvas's camera has its picture hold values, not
// light).
//
// The original's screen is 480 by 272 and its interface is drawn to it one texel to
// a pixel. Enlarged to a window, each texel is kept whole and only the pixel where
// two meet is a mix of them, which is that screen made bigger rather than smeared.
//
// What the original draws smaller than it is made, more than a texel to a pixel
// of its screen, is another matter. It draws everything with its textures
// smoothed, and its textures are made with that in mind: a glow has its fading
// done in a pattern of dots, and the text's letters are made half as big again
// as the small lines that are written with them. Kept whole, such texels show
// the dots, and letters whose strokes are now one pixel and now two. So what is
// drawn small is smoothed here too, each pixel a mix of the four texels about
// it, the more so the smaller it is drawn.

#import bevy_sprite::mesh2d_vertex_output::VertexOutput
#ifndef SRGB_OUTPUT
#import bevy_render::color_operations::srgb_to_linear
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var texture_sampler: sampler;

// How many texels to a pixel of the screen make a thing one that is drawn small,
// and how many more have it smoothed altogether.
const SMALL: f32 = 1.01;
const SMOOTHED_OVER: f32 = 0.24;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(texture));
    let texel = mesh.uv * size;
    // The texels gone by from one of the window's pixels to the next, across and
    // down.
    let over = dpdx(texel);
    let down = dpdy(texel);
    // The derivatives are those of the place asked for, so that no smaller copy of
    // the texture is taken where the texel changes.
    let uv_over = dpdx(mesh.uv);
    let uv_down = dpdy(mesh.uv);
    // How many of the window's pixels a texel is drawn across.
    let across = 1.0 / max(abs(over) + abs(down), vec2<f32>(0.000001));
    let nearest = floor(texel - 0.5) + 0.5;
    let mixed = clamp((texel - nearest - 0.5) * across + 0.5, vec2<f32>(0.0), vec2<f32>(1.0));
    var colour = textureSampleGrad(texture, texture_sampler, (nearest + mixed) / size, uv_over, uv_down);
#ifndef FINE
    // How much of a pixel of the original's screen, which are the canvas's whole
    // numbers, a pixel of the window is; and so how many texels go to one of the
    // screen's.
    let share = max(abs(vec2<f32>(dpdx(mesh.world_position.x), dpdy(mesh.world_position.y))), vec2<f32>(0.000001));
    let small = (max(length(over) / share.x, length(down) / share.y) - SMALL) / SMOOTHED_OVER;
    if small > 0.0 {
        // The four texels about this place, mixed by how near each is.
        let at = texel - 0.5;
        let first = floor(at);
        let part = at - first;
        let a = textureSampleGrad(texture, texture_sampler, (first + vec2<f32>(0.5, 0.5)) / size, uv_over, uv_down);
        let b = textureSampleGrad(texture, texture_sampler, (first + vec2<f32>(1.5, 0.5)) / size, uv_over, uv_down);
        let c = textureSampleGrad(texture, texture_sampler, (first + vec2<f32>(0.5, 1.5)) / size, uv_over, uv_down);
        let d = textureSampleGrad(texture, texture_sampler, (first + vec2<f32>(1.5, 1.5)) / size, uv_over, uv_down);
        colour = mix(colour, mix(mix(a, b, part.x), mix(c, d, part.x), part.y), min(small, 1.0));
    }
#endif
#ifdef VERTEX_COLORS
    colour = colour * mesh.color;
#endif
#ifndef SRGB_OUTPUT
    // A camera that mixes light is given the light the values stand for.
    colour = vec4(srgb_to_linear(colour.rgb), colour.a);
#endif
    return colour;
}
