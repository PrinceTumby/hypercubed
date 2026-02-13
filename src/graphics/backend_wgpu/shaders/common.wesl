import package::common_bind_group_idxs;

struct RenderInfo {
    view_matrix: mat4x4<f32>,
    sky_matrix: mat4x4<f32>,
    recip_screen_size: vec2<f32>,
    recip_block_item_atlas_size: vec2<f32>,
    face_matrices: array<mat3x3<f32>, 6>,
    time_of_day: f32,
    star_brightness: f32,
}

struct CustomBlockVertex {
    local_pos: array<f32, 3>,
    packed_uvs: u32,
    normal: array<f32, 3>,
    tint_percentage: f32,
}

@group(0) @binding(common_bind_group_idxs::RENDER_INFO)
var<uniform> render_info: RenderInfo;

@group(0) @binding(common_bind_group_idxs::LIGHTMAP)
var<storage, read> lightmap: array<array<u32, 16>, 16>;

@group(0) @binding(common_bind_group_idxs::BASIC_SAMPLER)
var basic_sampler: sampler;

@group(0) @binding(common_bind_group_idxs::BLOCK_ITEM_ATLAS)
var block_item_atlas_texture: texture_2d<f32>;

@group(0) @binding(common_bind_group_idxs::CUSTOM_BLOCK_FACES)
var<storage, read> custom_block_faces: array<array<CustomBlockVertex, 4>>;

@group(0) @binding(common_bind_group_idxs::SUN)
var sun_texture: texture_2d<f32>;

@group(0) @binding(common_bind_group_idxs::MOON_PHASES)
var moon_phases_texture: texture_2d<f32>;

const BASE_NORMAL: vec3<f32> = vec3(0.0, 1.0, 0.0);

fn get_base_position(vertex_i: u32) -> vec3<f32> {
    var out: vec3<f32>;
    switch vertex_i % 4 {
        case 0u: {
            out = vec3<f32>(-0.5, 0.5, 0.5);
        }
        case 1u: {
            out = vec3<f32>(0.5, 0.5, 0.5);
        }
        case 2u: {
            out = vec3<f32>(-0.5, 0.5, -0.5);
        }
        case 3u, default: {
            out = vec3<f32>(0.5, 0.5, -0.5);
        }
    }
    return out;
}

fn get_uvs(vertex_i: u32, face_rotation_i: u32) -> vec2<f32> {
    var uv_rotation_vec: vec4<u32>;
    switch face_rotation_i {
        case 0u: {
            uv_rotation_vec = vec4<u32>(0, 1, 2, 3);
        }
        case 1u: {
            uv_rotation_vec = vec4<u32>(1, 3, 0, 2);
        }
        case 2u: {
            uv_rotation_vec = vec4<u32>(3, 2, 1, 0);
        }
        case 3u, default: {
            uv_rotation_vec = vec4<u32>(2, 0, 3, 1);
        }
    }
    let new_vertex_i = uv_rotation_vec[vertex_i % 4];
    var out: vec2<f32>;
    switch new_vertex_i {
        case 0u: {
            out = vec2<f32>(0.0, 1.0);
        }
        case 1u: {
            out = vec2<f32>(1.0, 1.0);
        }
        case 2u: {
            out = vec2<f32>(0.0, 0.0);
        }
        case 3u, default: {
            out = vec2<f32>(1.0, 0.0);
        }
    }
    return out;
}
