struct BlockFaceInstance {
    packed_uvs: array<u32, 2>,
    /// 0-3: X offset
    /// 4-7: Y offset
    /// 8-11: Z offset
    /// 12-13: UV rotation
    /// 14: Emits light?
    /// 17-19: Unused
    /// 20-23: Sky light level
    /// 24-27: Block light level
    /// 28-31: Unused
    packed_fields: u32,
}

struct TintedBlockFaceInstance {
    packed_uvs: array<u32, 2>,
    tint_colour: u32,
    /// 0-3: X offset
    /// 4-7: Y offset
    /// 8-11: Z offset
    /// 12-13: UV rotation
    /// 14: Emits light?
    /// 17-19: Unused
    /// 20-23: Sky light level
    /// 24-27: Block light level
    /// 28-31: Unused
    packed_fields: u32,
}

struct CustomBlockVertex {
    pos: array<f32, 3>,
    packed_uvs: u32,
    normal: array<f32, 3>,
    /// 0: Tinted?
    /// 1-31: Unused
    packed_fields: u32,
}

struct CustomBlockInstance {
    pos: array<f32, 3>,
    tint_colour_rgba: u32,
    packed_light_level_pairs_and_fields: array<u32, 2>,
}

struct CustomBlockGroup {
    base_vertex: u32,
    indices: Slice,
    instances: Slice,
}

struct Slice {
    start: u32,
    len: u32,
}

struct SubchunkHashMapEntry {
    // Subchunk position, serves as the key.
    // Equal to `subchunk_entry_empty_key` if the entry is unused.
    pos: array<f32, 3>,
    // Block face start and length u32 pairs
    block_face_instance_slices: array<Slice, 6>,
    // Tinted lock face start and length u32 pairs
    tinted_block_face_instance_slices: array<Slice, 6>,
    custom_block_group_slice: Slice,
}

struct SubchunkHashMapLookupResult {
    entry_found: bool,
    block_face_instance_slices: array<Slice, 6>,
    tinted_block_face_instance_slices: array<Slice, 6>,
    custom_block_group_slice: Slice,
}

// Subchunk start positions are always integers, so this is valid to use as a sentinel.
const subchunk_entry_empty_key: vec3<f32> = vec3(0.1);

// Fraction of each texture atlas dimension that each square is.
// Calculated as `square_length / texture_atlas_dims`
@group(0) @binding(0)
var<uniform> block_item_atlas_size: vec2<f32>;
@group(0) @binding(1)
var block_item_atlas_texture: texture_2d<f32>;
@group(0) @binding(2)
var block_item_atlas_sampler: sampler;

@group(1) @binding(0)
var<uniform> face_matrices: array<mat3x3<f32>, 6>;

@group(2) @binding(0)
var<storage, read> subchunk_hash_map: array<SubchunkHashMapEntry>;
@group(2) @binding(1)
var<storage, read> block_face_instances: array<BlockFaceInstance>;
@group(2) @binding(2)
var<storage, read> tinted_block_face_instances: array<TintedBlockFaceInstance>;
@group(2) @binding(3)
var<storage, read> custom_block_vertices: array<CustomBlockVertex>;
@group(2) @binding(4)
var<storage, read> custom_block_indices: array<u32>;
@group(2) @binding(5)
var<storage, read> custom_block_instances: array<CustomBlockInstance>;
@group(2) @binding(6)
var<storage, read> custom_block_groups: array<CustomBlockGroup>;
// @group(2) @binding(8)
// var<storage, read_write> debug_info: DebugBufferInfo;

@group(3) @binding(0)
var output_image: texture_storage_2d<rgba8unorm, write>;

var<push_constant> update_info: UpdateInfo;

fn murmur_32_scramble(k: u32) -> u32 {
    var out = k;
    out *= 0xCC9E2D51u;
    out = (out << 15u) | (out >> 17u);
    out *= 0x1B873593u;
    return out;
}

fn subchunk_hash(key: vec3<f32>) -> u32 {
    // Hash components of key
    var hash: u32 = 0;
    hash ^= murmur_32_scramble(bitcast<u32>(key.x));
    hash = (hash << 13u) | (hash >> 19u);
    hash = hash * 5 + 0xE6546B64u;
    hash ^= murmur_32_scramble(bitcast<u32>(key.y));
    hash = (hash << 13u) | (hash >> 19u);
    hash = hash * 5 + 0xE6546B64u;
    hash ^= murmur_32_scramble(bitcast<u32>(key.z));
    hash = (hash << 13u) | (hash >> 19u);
    hash = hash * 5 + 0xE6546B64u;
    // Finalise hash
    hash ^= hash >> 16u;
    hash *= 0x85EBCA6Bu;
    hash ^= hash >> 13u;
    hash *= 0xC2B2AE35u;
    hash ^= hash >> 16u;
    return hash;
}

// fn subchunk_hash_map_lookup(
//     subchunk_pos: vec3<f32>,
// ) -> SubchunkHashMapLookupResult {
//     let hash_map_len = arrayLength(&subchunk_hash_map);
//     var out: SubchunkHashMapLookupResult;
//     out.entry_found = false;
//     var current_slot = subchunk_hash(subchunk_pos) % hash_map_len;
//     loop {
//         let entry = &subchunk_hash_map[current_slot];
//         if all((*entry).pos == subchunk_pos) {
//             out.block_face_instance_slices = (*entry).block_face_instance_slices;
//             out.tinted_block_face_instance_slices = (*entry).tinted_block_face_instance_slices;
//             out.entry_found = true;
//             return out;
//         } else if all((*entry).pos == subchunk_entry_empty_key) {
//             return out;
//         }
//         continuing {
//             // NOTE: The hash map must always contain at least one empty slot, otherwise lookup
//             // will loop forever if the key isn't in the hashmap.
//             current_slot++;
//             current_slot %= hash_map_len;
//         }
//     }
//     // Unreachable, but needed to pass verification
//     return out;
// }

fn get_probe_local_position(probe_i: u32) -> vec3<f32> {
    var out: vec3<f32>;
    switch probe_i % 4 {
        case 0u: {
            out = vec3<f32>(-0.25, 0.5, 0.25);
        }
        case 1u: {
            out = vec3<f32>(0.25, 0.5, 0.25);
        }
        case 2u: {
            out = vec3<f32>(-0.25, 0.5, -0.25);
        }
        case 3u, default: {
            out = vec3<f32>(0.25, 0.5, -0.25);
        }
    }
    return out;
}

fn get_ray_direction(ray_i: u32) -> vec3<f32> {
    var out: vec3<f32>;
    switch ray_i % 4 {
        case 0u: {
            out = vec3<f32>(-1.0, 1.0, 1.0);
        }
        case 1u: {
            out = vec3<f32>(1.0, 1.0, 1.0);
        }
        case 2u: {
            out = vec3<f32>(-1.0, 1.0, -1.0);
        }
        case 3u, default: {
            out = vec3<f32>(1.0, 1.0, -1.0);
        }
    }
    return normalize(out);
}

fn rotate_uvs(in: vec2<f32>, rotation: u32) -> vec2<f32> {
    let angle = 6.2831855 - (f32(rotation) * 1.5707964);
    let sin_angle = sin(angle);
    let cos_angle = cos(angle);
    let rotation_matrix = mat2x2(cos_angle, sin_angle, -sin_angle, cos_angle);
    return rotation_matrix * (in - 0.5) + 0.5;
}

struct UpdateInfo {
    inv_view_matrix: mat4x4<f32>,
}

struct DebugBufferInfo {
    debug_floats: vec4<f32>,
    debug_ints: vec4<u32>,
}

@compute
@workgroup_size(16, 4)
// @workgroup_size(1, 1)
fn render_raytraced(
    @builtin(global_invocation_id) invocation_id: vec3<u32>,
) {
    let texture_dims = textureDimensions(output_image);
    let camera_global_pos_v4 = update_info.inv_view_matrix * vec4(0.0, 0.0, 0.0, 1.0);
    let camera_global_pos = camera_global_pos_v4.xyz / camera_global_pos_v4.w;
    // Cast rays, collect hits
    var ray_results: array<vec4<f32>, 4>;
    // Ray info
    let ray_start = camera_global_pos;
    let ray_end_view_space_v4 = vec4(
        (f32(invocation_id.x) / f32(texture_dims.x)) * 2.0 - 1.0,
        (f32(invocation_id.y) / f32(texture_dims.y)) * 2.0 - 1.0,
        0.9999,
        1.0,
    );
    let ray_end_v4 = update_info.inv_view_matrix * ray_end_view_space_v4;
    let ray_end = ray_end_v4.xyz / ray_end_v4.w;
    let ray_diff = ray_end - ray_start;
    let ray_dir = normalize(ray_diff);
    let inv_ray_dir = vec3(1.0) / ray_dir;
    let ray_len = length(ray_diff);

    // DDA through subchunks
    let hash_map_len = arrayLength(&subchunk_hash_map);
    let subchunk_ray_start = ray_start / 16.0;
    let subchunk_ray_end = ray_end / 16.0;
    let subchunk_ray_dir = normalize(subchunk_ray_end - subchunk_ray_start);
    let subchunk_delta_distance = abs(vec3(length(subchunk_ray_dir)) / subchunk_ray_dir);
    let subchunk_ray_sign = sign(subchunk_ray_dir);
    var subchunk_pos = floor(subchunk_ray_start);
    var subchunk_side_distance = subchunk_ray_sign * (subchunk_pos - subchunk_ray_start);
    subchunk_side_distance += subchunk_ray_sign * 0.5;
    subchunk_side_distance += 0.5;
    subchunk_side_distance *= subchunk_delta_distance;
    var step_i: u32 = 0;
    var did_ray_hit = false;
    var ray_hit_len = 1.0;
    var ray_hit_pos: vec3<f32>;
    var ray_hit_normal: vec3<f32>;
    var ray_hit_colour: vec4<f32>;
    loop {
        // let subchunk_info = subchunk_hash_map_lookup(subchunk_pos);
        var subchunk_info: SubchunkHashMapLookupResult;
        {
            subchunk_info.entry_found = false;
            var current_slot = subchunk_hash(subchunk_pos) % hash_map_len;
            loop {
                let entry = &subchunk_hash_map[current_slot];
                let entry_pos_array = (*entry).pos;
                let entry_pos = vec3(
                    entry_pos_array[0],
                    entry_pos_array[1],
                    entry_pos_array[2],
                );
                if all(entry_pos == subchunk_pos) {
                    subchunk_info.block_face_instance_slices = (*entry).block_face_instance_slices;
                    subchunk_info.tinted_block_face_instance_slices = (*entry).tinted_block_face_instance_slices;
                    subchunk_info.custom_block_group_slice = (*entry).custom_block_group_slice;
                    subchunk_info.entry_found = true;
                    break;
                } else if all(entry_pos == subchunk_entry_empty_key) {
                    break;
                }
                continuing {
                    // NOTE: The hash map must always contain at least one empty slot, otherwise lookup
                    // will loop forever if the key doesn't exist.
                    current_slot++;
                    current_slot %= hash_map_len;
                }
            }
        }
        if !subchunk_info.entry_found {
            continue;
        }
        // Find first face that intersects ray in subchunk, if any
        for (var dir_i: u32 = 0; dir_i < 6; dir_i++) {
            // Backface culling
            let face_matrix = face_matrices[dir_i];
            // Face matrix is a rotation matrix, so we can invert by just transposing
            let inv_face_matrix = transpose(face_matrix);
            let face_normal = face_matrix * vec3(0.0, 1.0, 0.0);
            let denom = dot(ray_dir, face_normal);
            if denom >= 0.0 {
                continue;
            }
            // Test ray against block faces
            let instances = subchunk_info.block_face_instance_slices[dir_i];
            let instances_end = instances.start + instances.len;
            for (var instance_i = instances.start; instance_i < instances_end; instance_i++) {
                let instance = &block_face_instances[instance_i];
                let packed_fields = (*instance).packed_fields;
                let emits_light = ((packed_fields >> 14u) & 1u) != 0u;
                let x_offset = packed_fields & 0xFu;
                let y_offset = (packed_fields >> 4u) & 0xFu;
                let z_offset = (packed_fields >> 8u) & 0xFu;
                let offset_f32 = vec3<f32>(vec3<u32>(x_offset, y_offset, z_offset));
                let block_centre = fma(subchunk_pos, vec3(16.0), offset_f32) + vec3(0.5);
                // Corner of face at UV origin
                let face_base = face_matrix * vec3<f32>(-0.5, 0.5, 0.5) + block_centre;
                // Use face base to do a ray-plane intersection test
                let intersect_dist = dot(face_base - ray_start, face_normal) / denom;
                let intersect_hit_len = intersect_dist / ray_len;
                if (!did_ray_hit || intersect_hit_len < ray_hit_len)
                    && 0.0 <= intersect_dist
                    && intersect_dist < ray_len
                {
                    // Plane intersection is valid, find quad UV coordinates
                    let hit_pos = fma(ray_dir, vec3(intersect_dist), ray_start);
                    let hit_relative_3d = inv_face_matrix * (hit_pos - face_base);
                    let base_uvs = vec2(hit_relative_3d.x, -hit_relative_3d.z);
                    // If UVs are within range, we've hit the quad
                    if 0.0 <= base_uvs.x && base_uvs.x <= 1.0
                        && 0.0 <= base_uvs.y && base_uvs.y <= 1.0
                    {
                        did_ray_hit = true;
                        ray_hit_normal = face_normal;
                        ray_hit_len = intersect_dist / ray_len;
                        let packed_uvs = (*instance).packed_uvs;
                        let corrected_base_uvs = vec2(base_uvs.x, 1.0 - base_uvs.y);
                        let uv_rotation = (packed_fields >> 12u) & 0x3u;
                        let rotated_base_uvs = rotate_uvs(corrected_base_uvs, uv_rotation);
                        let start_coords = vec2(
                            f32(packed_uvs[0] & 0xFFFFu),
                            f32(packed_uvs[0] >> 16u),
                        );
                        let end_coords = vec2(
                            f32(packed_uvs[1] & 0xFFFFu),
                            f32(packed_uvs[1] >> 16u),
                        );
                        let coords_f32s = mix(start_coords, end_coords, rotated_base_uvs);
                        ray_hit_colour = textureLoad(
                            block_item_atlas_texture,
                            vec2<i32>(coords_f32s),
                            0,
                        );
                        // if emits_light {
                        //     ray_hit_colour = textureLoad(
                        //         block_item_atlas_texture,
                        //         vec2<i32>(coords_f32s),
                        //         0,
                        //     );
                        // } else {
                        //     ray_hit_colour = vec4(vec3(0.0), 1.0);
                        // }
                    }
                }
            }
            // Test ray against tinted block faces
            let tinted_instances = subchunk_info.tinted_block_face_instance_slices[dir_i];
            let tinted_instances_end = tinted_instances.start + tinted_instances.len;
            for (var inst_i = tinted_instances.start; inst_i < tinted_instances_end; inst_i++) {
                let instance = &tinted_block_face_instances[inst_i];
                let packed_fields = (*instance).packed_fields;
                let emits_light = ((packed_fields >> 14u) & 1u) != 0u;
                let x_offset = packed_fields & 0xFu;
                let y_offset = (packed_fields >> 4u) & 0xFu;
                let z_offset = (packed_fields >> 8u) & 0xFu;
                let offset_f32 = vec3<f32>(vec3<u32>(x_offset, y_offset, z_offset));
                let block_centre = fma(subchunk_pos, vec3(16.0), offset_f32) + vec3(0.5);
                // Corner of face at UV origin
                let face_base = face_matrix * vec3<f32>(-0.5, 0.5, 0.5) + block_centre;
                // Use face base to do a ray-plane intersection test
                let intersect_dist = dot(face_base - ray_start, face_normal) / denom;
                let intersect_hit_len = intersect_dist / ray_len;
                if (!did_ray_hit || intersect_hit_len <= ray_hit_len)
                    && 0.0 <= intersect_dist
                    && intersect_dist < ray_len
                {
                    // Plane intersection is valid, find quad UV coordinates
                    let hit_pos = fma(ray_dir, vec3(intersect_dist), ray_start);
                    let hit_relative_3d = inv_face_matrix * (hit_pos - face_base);
                    let base_uvs = vec2(hit_relative_3d.x, -hit_relative_3d.z);
                    // If UVs are within range, we've hit the quad
                    if 0.0 <= base_uvs.x && base_uvs.x <= 1.0
                        && 0.0 <= base_uvs.y && base_uvs.y <= 1.0
                    {
                        let packed_uvs = (*instance).packed_uvs;
                        let corrected_base_uvs = vec2(base_uvs.x, 1.0 - base_uvs.y);
                        let uv_rotation = (packed_fields >> 12u) & 0x3u;
                        let rotated_base_uvs = rotate_uvs(corrected_base_uvs, uv_rotation);
                        let start_coords = vec2(
                            f32(packed_uvs[0] & 0xFFFFu),
                            f32(packed_uvs[0] >> 16u),
                        );
                        let end_coords = vec2(
                            f32(packed_uvs[1] & 0xFFFFu),
                            f32(packed_uvs[1] >> 16u),
                        );
                        let coords_f32s = mix(start_coords, end_coords, rotated_base_uvs);
                        let base_texture_colour = textureLoad(
                            block_item_atlas_texture,
                            vec2<i32>(coords_f32s),
                            0,
                        );
                        if base_texture_colour.a == 1.0 {
                            did_ray_hit = true;
                            ray_hit_normal = face_normal;
                            ray_hit_len = intersect_dist / ray_len;
                            let tint_colour = unpack4x8unorm((*instance).tint_colour);
                            ray_hit_colour = base_texture_colour * tint_colour;
                            // if emits_light {
                            //     let tint_colour = unpack4x8unorm((*instance).tint_colour);
                            //     ray_hit_colour = base_texture_colour * tint_colour;
                            // } else {
                            //     ray_hit_colour = vec4(vec3(0.0), 1.0);
                            // }
                        }
                    }
                }
            }
        }

        // Test ray against custom block triangles
        let groups_slice = subchunk_info.custom_block_group_slice;
        let groups_end = groups_slice.start + groups_slice.len;
        for (var group_i = groups_slice.start; group_i < groups_end; group_i++) {
            let group = &custom_block_groups[group_i];
            let base_vertex_i = (*group).base_vertex;
            let instances = (*group).instances;
            let instances_end = instances.start + instances.len;
            for (var instance_i = instances.start; instance_i < instances_end; instance_i++) {
                let instance = custom_block_instances[instance_i];
                let instance_pos = vec3(
                    instance.pos[0],
                    instance.pos[1],
                    instance.pos[2],
                );
                let packed_fields = instance.packed_light_level_pairs_and_fields[1] >> 24u;
                let emits_light = (packed_fields & 1u) != 0;
                // Test ray against AABB, skip entire instance if not intersecting
                {
                    let aabb_min = instance_pos;
                    let aabb_max = instance_pos + vec3(1.0);
                    let t1 = (aabb_min - ray_start) * inv_ray_dir;
                    let t2 = (aabb_max - ray_start) * inv_ray_dir;
                    var tmin = min(t1.x, t2.x);
                    tmin = max(tmin, min(t1.y, t2.y));
                    tmin = max(tmin, min(t1.z, t2.z));
                    var tmax = max(t1.x, t2.x);
                    tmax = min(tmax, max(t1.y, t2.y));
                    tmax = min(tmax, max(t1.z, t2.z));
                    let intersect_hit_len = tmin / ray_len;
                    if tmax < 0.0 || tmax < tmin || intersect_hit_len >= ray_hit_len {
                        continue;
                    }
                }
                let indices = (*group).indices;
                let indices_end = indices.start + indices.len;
                for (var index_i = indices.start; index_i < indices_end - 2; index_i += 3u) {
                    let index_0 = custom_block_indices[index_i];
                    let index_1 = custom_block_indices[index_i + 1];
                    let index_2 = custom_block_indices[index_i + 2];
                    let vertex_0 = custom_block_vertices[base_vertex_i + index_0];
                    let vertex_1 = custom_block_vertices[base_vertex_i + index_1];
                    let vertex_2 = custom_block_vertices[base_vertex_i + index_2];
                    // Convert to global space
                    let vertex_0_pos = vec3(
                        vertex_0.pos[0],
                        vertex_0.pos[1],
                        vertex_0.pos[2],
                    ) + instance_pos + vec3(0.5);
                    let vertex_1_pos = vec3(
                        vertex_1.pos[0],
                        vertex_1.pos[1],
                        vertex_1.pos[2],
                    ) + instance_pos + vec3(0.5);
                    let vertex_2_pos = vec3(
                        vertex_2.pos[0],
                        vertex_2.pos[1],
                        vertex_2.pos[2],
                    ) + instance_pos + vec3(0.5);
                    // Calculate triangle barycentric coordinates
                    let e1 = vertex_1_pos - vertex_0_pos;
                    let e2 = vertex_2_pos - vertex_0_pos;
                    let ray_cross_e2 = cross(ray_dir, e2);
                    let det = dot(e1, ray_cross_e2);
                    if det < 0.0001 {
                        // Ray is parallel to triangle
                        continue;
                    }
                    let inv_det = 1.0 / det;
                    let s = ray_start - vertex_0_pos;
                    let u = inv_det * dot(s, ray_cross_e2);
                    if u < 0.0 || u > 1.0 {
                        continue;
                    }
                    let s_cross_e1 = cross(s, e1);
                    let v = inv_det * dot(ray_dir, s_cross_e1);
                    let w = 1.0 - u - v;
                    if v < 0.0 || w < 0.0 {
                        continue;
                    }
                    // We're definitely in the triangle, so now we can check ray bounds
                    let intersect_dist = inv_det * dot(e2, s_cross_e1);
                    let intersect_hit_len = intersect_dist / ray_len;
                    if (!did_ray_hit || intersect_hit_len <= ray_hit_len)
                        && 0.0 <= intersect_dist
                        && intersect_dist < ray_len
                    {
                        // Interpolate texture coordinates
                        let vertex_0_uvs = vec2<f32>(vec2(
                            vertex_0.packed_uvs & 0xFFFFu,
                            vertex_0.packed_uvs >> 16u,
                        ));
                        let vertex_1_uvs = vec2<f32>(vec2(
                            vertex_1.packed_uvs & 0xFFFFu,
                            vertex_1.packed_uvs >> 16u,
                        ));
                        let vertex_2_uvs = vec2<f32>(vec2(
                            vertex_2.packed_uvs & 0xFFFFu,
                            vertex_2.packed_uvs >> 16u,
                        ));
                        let uv_coords = vertex_0_uvs * w + vertex_1_uvs * u + vertex_2_uvs * v;
                        let base_texture_colour = textureLoad(
                            block_item_atlas_texture,
                            vec2<i32>(uv_coords),
                            0,
                        );
                        if base_texture_colour.a == 1.0 {
                            did_ray_hit = true;
                            ray_hit_len = intersect_hit_len;
                            let tint_colour = unpack4x8unorm(instance.tint_colour_rgba);
                            let tint_percentage = f32(vertex_0.packed_fields & 0x1u);
                            let tint = mix(vec4(1.0), tint_colour, tint_percentage);
                            ray_hit_colour = base_texture_colour * tint;
                            // if emits_light {
                            //     ray_hit_colour = base_texture_colour * tint;
                            // } else {
                            //     ray_hit_colour = vec4(vec3(0.0), 1.0);
                            // }
                        }
                    }
                }
            }
        }

        if did_ray_hit {
            break;
        }

        continuing {
            let mask = subchunk_side_distance.xyz <=
                min(subchunk_side_distance.yzx, subchunk_side_distance.zxy);
            subchunk_side_distance += vec3<f32>(mask) * subchunk_delta_distance;
            subchunk_pos += vec3<f32>(mask) * subchunk_ray_sign;
            step_i++;
            break if step_i > 4;
        }
    }

    var output_colour: vec4<f32>;
    if did_ray_hit {
        // output_colour = vec4(ray_hit_normal * 0.5 + 0.5, 1.0);
        output_colour = ray_hit_colour;
    } else {
        output_colour = vec4(0.0);
    }
    let store_y = texture_dims.y - 1 - invocation_id.y;
    textureStore(output_image, vec2(invocation_id.x, store_y), output_colour);
}
