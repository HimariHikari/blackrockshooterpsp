// What a particle of an effect is drawn with: its picture by its colour, with
// neither light nor fog on it (0x127CD8), both faces of it (0x127D18).
//
// The colour is the particle's own, which every corner of it has: it comes with
// the mesh's tag, a byte each of red, green, blue and alpha.

#import bevy_pbr::mesh_functions::{get_tag, get_world_from_local, mesh_position_local_to_clip, mesh_position_local_to_world}
#import bevy_pbr::view_transformations::position_world_to_view

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var picture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var picture_sampler: sampler;
// How far before the camera the lens it is drawn by sees, and from how near:
// nothing of it is drawn beyond the first or nearer than the second, and nought
// is no end.
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> reach: vec4<f32>;

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
    @location(2) before: f32,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let tag = get_tag(vertex.instance_index);
    let own = vec4<f32>(f32(tag & 0xffu), f32((tag >> 8u) & 0xffu), f32((tag >> 16u) & 0xffu), f32(tag >> 24u)) / 255.0;
#ifdef OWN_COLOUR_TWICE
    // A model is lit by nothing but a light all about it of the particle's colour,
    // and what takes that light is its corners' colours; one whose corners have
    // none takes it by the particle's colour again (0x122740).
    out.colour = own * own;
#else
    out.colour = own * vertex.colour;
#endif
    let world_from_local = get_world_from_local(vertex.instance_index);
    out.clip_position = mesh_position_local_to_clip(world_from_local, vec4<f32>(vertex.position, 1.0));
    out.before = -position_world_to_view(mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0)).xyz).z;
    out.uv = vertex.uv;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    if (reach.x > 0.0 && in.before > reach.x) || (reach.y > 0.0 && in.before < reach.y) {
        discard;
    }
    return textureSample(picture, picture_sampler, in.uv) * in.colour;
}
