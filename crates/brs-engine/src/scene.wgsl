// What the picture of the scene is laid down with, under the canvas: its colours
// as the values the screen is given, which is what the canvas is mixed with.
//
// With the glare on it. The game draws its screen into a picture a quarter of its
// size, smoothed, by the glare's strength, and draws that back over the whole
// screen, smoothed again, added to what is there (0x19F4C, 0x18B70). A point of
// the small picture is the screen about the middle of its four-by-four block, as
// the PSP's smoothing takes it: the four points that meet there. It is worked
// out on the game's own numbers, which a point of the picture is read back from
// here as the light they stand for.

#import bevy_sprite::mesh2d_vertex_output::VertexOutput
#import bevy_render::color_operations::{linear_to_srgb, srgb_to_linear}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var scene: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var scene_sampler: sampler;
// How strong the glare is; and a pixel of the game's screen, across and down, as
// parts of the picture.
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> glare: vec4<f32>;
// Where the game's screen begins in the picture.
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> corner: vec4<f32>;

// The game's numbers at a place of the picture.
fn numbers(at: vec2<f32>) -> vec3<f32> {
    return linear_to_srgb(textureSampleLevel(scene, scene_sampler, at, 0.0).rgb);
}

// A point of the small picture, by the middle of its block in the game's pixels.
fn small(middle: vec2<f32>) -> vec3<f32> {
    let at = corner.xy + middle * glare.yz;
    let half = glare.yz * 0.5;
    return 0.25 * (numbers(at + vec2<f32>(-half.x, -half.y)) + numbers(at + vec2<f32>(half.x, -half.y)) + numbers(at + vec2<f32>(-half.x, half.y)) + numbers(at + half));
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let drawn = textureSample(scene, scene_sampler, mesh.uv);
    var colour = drawn.rgb;
    if glare.x > 0.0 {
        // Where this is among the small picture's points, whose middles are every
        // four of the game's pixels from two in.
        let among = (mesh.uv - corner.xy) / glare.yz / 4.0 - vec2<f32>(0.5);
        let first = floor(among);
        let part = among - first;
        let middle = (first + vec2<f32>(0.5)) * 4.0;
        let upper = mix(small(middle), small(middle + vec2<f32>(4.0, 0.0)), part.x);
        let lower = mix(small(middle + vec2<f32>(0.0, 4.0)), small(middle + vec2<f32>(4.0, 4.0)), part.x);
        let lit = min(linear_to_srgb(colour) + mix(upper, lower, part.y) * glare.x, vec3<f32>(1.0));
        colour = srgb_to_linear(lit);
    }
#ifdef SRGB_OUTPUT
    colour = linear_to_srgb(colour);
#endif
    // How much of it is there matters only where the picture is laid over
    // something: what the scenes' cameras left clear lets that through.
    return vec4<f32>(colour, drawn.a);
}
