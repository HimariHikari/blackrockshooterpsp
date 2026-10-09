// What a map's geometry is drawn with: its picture by its corners' colours, which
// hold its lighting, as the PSP does it.
//
// The picture is taken by the corner's colour (which the mesh holds twice over
// where the material says so) or, in a few lists, added to it; how much of a
// point there is is the corner's alpha, by the picture's where that counts; a
// point with less than the material's least is not drawn at all; what comes of it
// is cut off at full, and then mixed with the fog's colour by how deep in the
// view it lies. How that is put on what is there is the pipeline's.

#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_world}
#import bevy_pbr::view_transformations::{position_world_to_clip, position_world_to_view}
#ifdef DISTANCE_FOG
#import bevy_pbr::mesh_view_bindings::fog
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var picture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var picture_sampler: sampler;
// The least alpha that is drawn; bits (1 the picture's alpha counts, 2 the picture
// is added, 4 what is drawn is all there); what an added picture is taken by; and
// whether the fog is on it.
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> look: vec4<f32>;
// How far before the camera its lens sees, and from how near: nothing of it is
// drawn beyond the first (nought is no end) or nearer than the second.
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> reach: vec4<f32>;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) colour: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) colour: vec4<f32>,
    @location(2) depth: f32,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world = mesh_position_local_to_world(get_world_from_local(vertex.instance_index), vec4<f32>(vertex.position, 1.0));
    out.clip_position = position_world_to_clip(world.xyz);
    out.uv = vertex.uv;
    out.colour = vertex.colour;
    // How far before the camera it lies, which is what the PSP's fog goes by.
    out.depth = -position_world_to_view(world.xyz).z;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    if (reach.x > 0.0 && in.depth > reach.x) || in.depth < reach.y {
        discard;
    }
    let texel = textureSample(picture, picture_sampler, in.uv);
    let bits = u32(look.y + 0.5);
    var alpha = in.colour.a;
    if (bits & 1u) != 0u {
        alpha = alpha * texel.a;
    }
    var colour = in.colour.rgb * texel.rgb;
    if (bits & 2u) != 0u {
        colour = in.colour.rgb + texel.rgb * look.z;
    }
    if alpha < look.x {
        discard;
    }
    colour = min(colour, vec3<f32>(1.0));
    if (bits & 4u) != 0u {
        alpha = 1.0;
    }
#ifdef DISTANCE_FOG
    if look.w > 0.5 {
        let share = (1.0 - clamp((fog.be.y - in.depth) / (fog.be.y - fog.be.x), 0.0, 1.0)) * fog.base_color.a;
        colour = mix(colour, fog.base_color.rgb, share);
    }
#endif
    return vec4<f32>(colour, alpha);
}
