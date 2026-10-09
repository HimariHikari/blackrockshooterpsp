// A model that takes a light as the PSP gave it one: by its corners, with one light
// that shines from a way and the light that is all about the scene.
//
// What a corner's colour comes to is the sum of three (the PSP's own lighting, with
// the camera taken to be far off for the shine):
//   what is all about, by the material's ambient colour;
//   the light's own colour, by the material's diffuse colour and by how far the
//   corner faces the light;
//   the light's shine, by the material's specular colour and by how far the corner
//   faces half way between the light and the camera, raised to the material's power.
// Which of the material's colours are the corner's own colour is the material's say
// (the rest are white). The sum is held between nothing and one, and the picture is
// put on it as the material says: by it or added to it, and twice over or not. How
// much of it there is, is its corner's by how much there is of the light all about
// (the PSP's sum for alpha), which is how much the game has there be of the model.
//
// Here it is worked out at every point of a face and not at its corners alone.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::view,
    view_transformations::position_world_to_view,
}

struct Light {
    // What is all about the scene and the light's own share of that, summed; and
    // how much there is of the model.
    ambient: vec4<f32>,
    diffuse: vec4<f32>,
    // The colour of the shine, and with it the power the material raises it to.
    specular: vec4<f32>,
    // The way to the light, in the world; and whether it is lit at all.
    towards: vec4<f32>,
    // Which of its colours are its corners' (a bit each: ambient, diffuse,
    // specular), how its picture is put on (4 is added), and whether twice over.
    look: vec4<f32>,
    // How far before the camera the lens it is drawn by sees (nought is no end),
    // and from how near.
    reach: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> light: Light;

@fragment
fn fragment(vertex_output: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var in = vertex_output;
    let before = -position_world_to_view(in.world_position.xyz).z;
    if (light.reach.x > 0.0 && before > light.reach.x) || before < light.reach.y {
        discard;
    }
#ifdef VERTEX_COLORS
    let corner = in.color;
    in.color = vec4<f32>(1.0);
#else
    let corner = vec4<f32>(1.0);
#endif
    // With the corner's colour taken out of it this is the picture alone.
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let picture = pbr_input.material.base_color;

    var colour = corner.rgb;
    var there = corner.a;
    if light.towards.w > 0.5 {
        there = corner.a * light.ambient.a;
        let own = u32(light.look.x + 0.5);
        let of_ambient = select(vec3<f32>(1.0), corner.rgb, (own & 1u) != 0u);
        let of_diffuse = select(vec3<f32>(1.0), corner.rgb, (own & 2u) != 0u);
        let of_specular = select(vec3<f32>(1.0), corner.rgb, (own & 4u) != 0u);
        // The PSP lights a face by the way its corners face, whichever side is seen.
        let normal = normalize(in.world_normal);
        let towards = light.towards.xyz;
        var sum = of_ambient * light.ambient.rgb + of_diffuse * light.diffuse.rgb * max(dot(normal, towards), 0.0);
        // The camera is taken to be far off the way it looks from.
        let half_way = normalize(towards + normalize(view.world_from_view[2].xyz));
        let facing = dot(normal, half_way);
        if facing > 0.0 {
            sum += of_specular * light.specular.rgb * pow(facing, light.specular.w);
        }
        colour = clamp(sum, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    var mixed = colour * picture.rgb;
    if light.look.y > 3.5 {
        mixed = colour + picture.rgb;
    }
    if light.look.z > 0.5 {
        mixed = mixed * 2.0;
    }
    pbr_input.material.base_color = vec4<f32>(clamp(mixed, vec3<f32>(0.0), vec3<f32>(1.0)), there * picture.a);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr_input, pbr_input.material.base_color);
    return out;
}
