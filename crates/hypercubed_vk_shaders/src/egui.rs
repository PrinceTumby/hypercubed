use spirv_std::glam::{Vec2, Vec4, vec4};
use spirv_std::image::{Image2d, SampledImage};
use spirv_std::spirv;

pub struct VertexOutput {
    uvs: Vec2,
    colour: Vec4,
}

#[spirv(vertex)]
pub fn vertex(
    // Bindings
    #[spirv(uniform, descriptor_set = 0, binding = 0)] screen_size: &Vec2,
    // Inputs
    pos: Vec2,
    uvs: Vec2,
    colour: Vec4,
    // Outputs
    #[spirv(position)] output_pos: &mut Vec4,
    output_data: &mut VertexOutput,
) {
    let screen_pos = pos / *screen_size;
    *output_pos = vec4(screen_pos.x * 2.0 - 1.0, screen_pos.y * 2.0 - 1.0, 0.0, 1.0);
    *output_data = VertexOutput { uvs, colour };
}

#[spirv(fragment)]
pub fn fragment(
    #[spirv(descriptor_set = 1, binding = 0)] image: &SampledImage<Image2d>,
    input: VertexOutput,
    output_colour: &mut Vec4,
) {
    *output_colour = image.sample(input.uvs) * input.colour;
}
