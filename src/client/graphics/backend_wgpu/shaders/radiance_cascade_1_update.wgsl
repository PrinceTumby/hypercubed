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
@group(2) @binding(7)
var<storage, read> updates: array<UpdateInfo>;
@group(2) @binding(8)
// var<storage, read_write> output_lightmap: array<array<array<atomic<u32>, 2>, 64>>;
var<storage, read_write> output_lightmap: array<array<array<u32, 2>, 64>>;
@group(2) @binding(10)
var block_item_atlas_luma_texture: texture_2d<f32>;

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

fn get_probe_local_position(probe_i: u32) -> vec3<f32> {
    let probe_x = probe_i % 8;
    let probe_z = probe_i / 8;
    let probe_x_pos = fma(f32(probe_x), 0.875 / 8.0, 1.0 / 32.0);
    let probe_z_pos = fma(f32(probe_z), 0.875 / 8.0, 1.0 / 32.0);
    return vec3<f32>(probe_x_pos - 0.5, 0.5, 0.5 - probe_z_pos);
}

fn get_ray_direction(ray_i: u32) -> vec3<f32> {
    let phi = 3.1415927 * (sqrt(5.0) - 1.0);
    let y = 1.0 - (f32(ray_i) / f32(num_rays * 2 - 1)) * 2.0;
    let radius = sqrt(1.0 - (y * y));
    let theta = phi * f32(ray_i);
    let x = cos(theta) * radius;
    let z = sin(theta) * radius;
    return normalize(vec3<f32>(x, y, z));
}

fn get_cascade_0_ray_direction(ray_i: u32) -> vec3<f32> {
    var out: vec3<f32>;
    switch ray_i {
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

struct UpdateInfo {
    subchunk_start_coords: array<f32, 3>,
    faces_start: u32,
    faces_len: u32,
    faces_dir_i: u32,
}

struct DebugBufferInfo {
    debug_floats: vec4<f32>,
    debug_ints: vec4<u32>,
}

fn rotate_uvs(in: vec2<f32>, rotation: u32) -> vec2<f32> {
    let angle = 6.2831855 - (f32(rotation) * 1.5707964);
    let sin_angle = sin(angle);
    let cos_angle = cos(angle);
    let rotation_matrix = mat2x2(cos_angle, sin_angle, -sin_angle, cos_angle);
    return rotation_matrix * (in - 0.5) + 0.5;
}

fn rgb_to_hsv448(in: vec3<f32>) -> u32 {
    let k = vec4(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p: vec4<f32> = mix(vec4(in.bg, k.wz), vec4(in.gb, k.xy), step(in.b, in.g));
    let q: vec4<f32> = mix(vec4(p.xyw, in.r), vec4(in.r, p.yzx), step(p.x, in.r));
    let d: f32 = q.x - min(q.w, q.y);
    let e: f32 = 1.0e-10;
    let h = abs(q.z + (q.w - q.y) / (6.0 * d + e));
    let s = d / (q.x + e);
    let v = q.x;
    return u32(round(clamp(h, 0.0, 1.0) * 15.0))
        | (u32(round(clamp(s, 0.0, 1.0) * 15.0)) << 4u)
        | (u32(round(clamp(v, 0.0, 1.0) * 255.0)) << 8u);
}

const cascade_0_ray_length: f32 = 1.0 / 16.0;
const cascade_ray_length: f32 = cascade_0_ray_length * 8.0;
// const num_rays: u32 = 4 * 8;
const num_rays: u32 = 256;

@compute
@workgroup_size(1, 64)
fn update_cascade(
    @builtin(global_invocation_id) invocation_id: vec3<u32>,
    @builtin(workgroup_id) global_workgroup_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    let update_info = updates[global_workgroup_id.y];
    if invocation_id.x >= update_info.faces_len {
        return;
    }
    // We're casting rays from a probe in a block face, so first we need the face info to calculate
    // the probe info.
    let update_face_matrix = face_matrices[update_info.faces_dir_i];
    let update_face_i = update_info.faces_start + invocation_id.x;
    let update_face_packed_fields = block_face_instances[update_face_i].packed_fields;
    let update_face_x_offset = update_face_packed_fields & 0xF;
    let update_face_y_offset = (update_face_packed_fields >> 4u) & 0xF;
    let update_face_z_offset = (update_face_packed_fields >> 8u) & 0xF;
    let update_face_offset_f32 = vec3<f32>(vec3<u32>(
        update_face_x_offset,
        update_face_y_offset,
        update_face_z_offset,
    ));
    let subchunk_start_coords = vec3(
        update_info.subchunk_start_coords[0],
        update_info.subchunk_start_coords[1],
        update_info.subchunk_start_coords[2],
    );
    let update_face_block_centre = subchunk_start_coords + update_face_offset_f32 + vec3(0.5);
    let probe_i = local_id.y;
    let probe_face_local_pos = get_probe_local_position(probe_i);
    let probe_block_local_pos = update_face_matrix * probe_face_local_pos;
    let probe_global_pos = update_face_block_centre + probe_block_local_pos;

    // Cast rays, collect hits
    var ray_results: array<vec4<f32>, num_rays>;
    for (var ray_i: u32 = 0; ray_i < num_rays; ray_i++) {
        // Ray info
        let ray_dir = update_face_matrix * get_ray_direction(ray_i);
        // let ray_len = cascade_ray_length - cascade_0_ray_length;
        // let ray_start = fma(ray_dir, vec3(cascade_0_ray_length), probe_global_pos);
        let ray_len = cascade_ray_length;
        let ray_start = probe_global_pos;

        let ray_hit_colour = raycast(ray_start, ray_dir, ray_len);
        let facing_sun = step(0.95, dot(ray_dir, normalize(vec3(1.0, 4.0, 2.0))));
        // let base_colour = mix(vec4(vec3(0.75), 1.0), vec4(1.0), facing_sun);
        let base_colour = vec4(vec3(0.0), 1.0);
        ray_results[ray_i] = mix(base_colour, ray_hit_colour, ray_hit_colour.a);
    }

    // Pre-average results into cascade 0 rays
    var reduced_ray_results: array<vec4<f32>, 4>;
    for (var reduced_ray_i: u32 = 0; reduced_ray_i < 4; reduced_ray_i++) {
        let reduced_ray_dir = get_cascade_0_ray_direction(reduced_ray_i);
        var average = vec4(0.0);
        var dot_total = 0.0;
        for (var ray_i: u32 = 0; ray_i < num_rays; ray_i++) {
            let ray_dir = get_ray_direction(ray_i);
            let facing_coef = clamp(dot(reduced_ray_dir, ray_dir), 0.0, 1.0);
            average += ray_results[ray_i] * facing_coef;
            dot_total += facing_coef;
        }
        reduced_ray_results[reduced_ray_i] = average / dot_total;
    }

    // Store results
    // for (var reduced_ray_i: u32 = 0; reduced_ray_i < 4; reduced_ray_i++) {
    //     var mask: u32;
    //     var shift_amount: u32;
    //     if reduced_ray_i % 2 == 0 {
    //         mask = 0x0000FFFFu;
    //         shift_amount = 0u;
    //     } else {
    //         mask = 0xFFFF0000u;
    //         shift_amount = 16u;
    //     }
    //     let ray_pair = &output_lightmap[update_face_i][probe_i][reduced_ray_i / 2];
    //     let ray_result = reduced_ray_results[reduced_ray_i];
    //     // let ray_result = vec4(1.0);
    //     atomicAnd(ray_pair, ~mask);
    //     atomicOr(ray_pair, rgb_to_hsv448(ray_result.rgb) << shift_amount);
    // }
    for (var reduced_ray_i: u32 = 0; reduced_ray_i < 4; reduced_ray_i += 2u) {
        let packed_ray_pair = rgb_to_hsv448(reduced_ray_results[reduced_ray_i].rgb)
            | (rgb_to_hsv448(reduced_ray_results[reduced_ray_i + 1].rgb) << 16u);
        output_lightmap[update_face_i][probe_i][reduced_ray_i / 2] = packed_ray_pair;
    }
}

fn raycast(
    ray_start: vec3<f32>,
    ray_dir: vec3<f32>,
    ray_len: f32,
) -> vec4<f32> {
    let ray_end = fma(ray_dir, vec3(ray_len), ray_start);
    let inv_ray_dir = vec3(1.0) / ray_dir;
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
    var cur_pos: vec3<f32> = ray_start;
    var did_ray_hit = false;
    var ray_hit_len: f32 = 1.0;
    // var ray_hit_colour: vec4<f32> = vec4(vec3(1.0), 1.0);
    var ray_hit_colour: vec4<f32> = vec4(0.0);
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
                        if emits_light {
                            let base_texture_colour = textureLoad(
                                block_item_atlas_texture,
                                vec2<i32>(coords_f32s),
                                0,
                            );
                            let luma_multiplier = textureLoad(
                                block_item_atlas_luma_texture,
                                vec2<i32>(coords_f32s),
                                0,
                            ).rrr * 16.0;
                            ray_hit_colour = base_texture_colour * vec4(luma_multiplier, 1.0);
                        } else {
                            ray_hit_colour = vec4(vec3(0.0), 1.0);
                        }
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
                            ray_hit_len = intersect_dist / ray_len;
                            if emits_light {
                                let tint_colour = unpack4x8unorm((*instance).tint_colour);
                                let luma_multiplier = textureLoad(
                                    block_item_atlas_luma_texture,
                                    vec2<i32>(coords_f32s),
                                    0,
                                ).rrr * 16.0;
                                ray_hit_colour = base_texture_colour
                                    * tint_colour
                                    * vec4(luma_multiplier, 1.0);
                            } else {
                                ray_hit_colour = vec4(vec3(0.0), 1.0);
                            }
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
                            let base_tint = unpack4x8unorm(instance.tint_colour_rgba);
                            let tint_percentage = f32(vertex_0.packed_fields & 0x1u);
                            let tint_colour = mix(vec4(1.0), base_tint, tint_percentage);
                            if emits_light {
                                let luma_multiplier = textureLoad(
                                    block_item_atlas_luma_texture,
                                    vec2<i32>(uv_coords),
                                    0,
                                ).rrr * 16.0;
                                ray_hit_colour = base_texture_colour
                                    * tint_colour
                                    * vec4(luma_multiplier, 1.0);
                            } else {
                                ray_hit_colour = vec4(vec3(0.0), 1.0);
                            }
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
    return ray_hit_colour;
}
