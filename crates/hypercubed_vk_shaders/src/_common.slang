#language slang 2026

module common;

// Functions

public float3 get_base_position(uint32_t vertex_i) {
    switch (vertex_i % 4) {
        case 0: return float3(-0.5, 0.5, 0.5);
        case 1: return float3(0.5, 0.5, 0.5);
        case 2: return float3(-0.5, 0.5, -0.5);
        default: return float3(0.5, 0.5, -0.5);
    }
}

public float2 get_uvs(uint vertex_i, uint32_t face_rotation_i) {
    let uv_rotations: uint[4][4] = {
        {0, 1, 2, 3},
        {1, 3, 0, 2},
        {3, 2, 1, 0},
        {2, 0, 3, 1},
    };
    let new_vertex_i = uv_rotations[face_rotation_i][vertex_i % 4];
    let uvs: float2[4] = {
        float2(0.0, 1.0),
        float2(1.0, 1.0),
        float2(0.0, 0.0),
        float2(1.0, 0.0),
    };
    return uvs[new_vertex_i];
}

// Types

public struct RenderInfo {
    public float4x4 view_matrix;
    public float4x4 sky_matrix;
    public float2 recip_screen_size;
    public float2 recip_block_item_atlas_size;
    public float[4][3][6] raw_face_matrices;
    public float time_of_day;
    public float star_brightness;
    
    public property float3x3[6] face_matrices {
        get {
            float3x3[6] out;
            for (int i = 0; i < 6; i++) {
                let m = raw_face_matrices[i];
                out[i] = float3x3(
                    // m[0][0], m[0][1], m[0][2],
                    // m[1][0], m[1][1], m[1][2],
                    // m[2][0], m[2][1], m[2][2],
                    m[0][0], m[1][0], m[2][0],
                    m[0][1], m[1][1], m[2][1],
                    m[0][2], m[1][2], m[2][2],
                );
            }
            return out;
        }
    }
}

public struct SubchunkFaceGroupInfo<T> {
    public T* instances;
    public int32_t[3] subchunk_start_coords;
    public uint32_t face_matrix_index;
    
    public property float3 subchunk_start_coords_float3 {
        get {
            return (float3)int32_t3(
                subchunk_start_coords[0],
                subchunk_start_coords[1],
                subchunk_start_coords[2],
            );
        }
    }
}

public struct BlockFaceInstanceFields {
    public uint32_t data;

    public property uint32_t x_offset {
        get { return data & 0xF; }
    }

    public property uint32_t y_offset {
        get { return (data >> 4) & 0xF; }
    }

    public property uint32_t z_offset {
        get { return (data >> 8) & 0xF; }
    }

    public property float3 xyz_offset {
        get {
            return (float3)uint32_t3(
                x_offset,
                y_offset,
                z_offset,
            );
        }
    }

    public property uint32_t uv_rotation {
        get { return (data >> 12) & 0b11; }
    }

    public property uint32_t sky_light_level {
        get { return (data >> 20) & 0xF; }
    }

    public property uint32_t block_light_level {
        get { return (data >> 24) & 0xF; }
    }
}

public struct BlockFaceInstance {
    public uint32_t[2] packed_uvs;
    public uint32_t raw_packed_fields;

    public property uint4 uvs {
        get {
            return uint4(
                packed_uvs[0] & 0xFFFF,
                (packed_uvs[0] >> 16) & 0xFFFF,
                packed_uvs[1] & 0xFFFF,
                (packed_uvs[1] >> 16) & 0xFFFF,
            );
        }
    }

    public property BlockFaceInstanceFields packed_fields {
        get {
            return BlockFaceInstanceFields(raw_packed_fields);
        }
    }
}

public struct TintedBlockFaceInstance {
    public uint32_t[2] packed_uvs;
    public uint32_t raw_tint_colour;
    public uint32_t raw_packed_fields;

    public property uint4 uvs {
        get {
            return uint4(
                packed_uvs[0] & 0xFFFF,
                (packed_uvs[0] >> 16) & 0xFFFF,
                packed_uvs[1] & 0xFFFF,
                (packed_uvs[1] >> 16) & 0xFFFF,
            );
        }
    }

    public property float4 tint_colour {
        get {
            return unpackUnorm4x8ToFloat(raw_tint_colour);
        }
    }

    public property BlockFaceInstanceFields packed_fields {
        get {
            return BlockFaceInstanceFields(raw_packed_fields);
        }
    }
}

public struct CustomBlockVertexFields {
    public uint32_t data;

    public property bool is_tinted {
        get { return (data & 0x1) != 0; }
    }

    public property float tint_percentage {
        get { return is_tinted ? 1.0 : 0.0; }
    }
}

public struct CustomBlockVertex {
    public float[3] raw_pos;
    public uint32_t raw_uvs;
    public float[3] raw_normal;
    public uint32_t raw_packed_fields;

    public property float3 pos {
        get {
            return float3(
                raw_pos[0],
                raw_pos[1],
                raw_pos[2],
            );
        }
    }

    public property uint2 uvs {
        get {
            return uint2(
                raw_uvs & 0xFFFF,
                (raw_uvs >> 16) & 0xFFFF,
            );
        }
    }

    public property float3 normal {
        get {
            return float3(
                raw_normal[0],
                raw_normal[1],
                raw_normal[2],
            );
        }
    }

    public property CustomBlockVertexFields packed_fields {
        get {
            return CustomBlockVertexFields(raw_packed_fields);
        }
    }
}

public struct CustomBlockInstance {
    public float[3] raw_pos;
    public uint32_t raw_tint_colour;
    public uint32_t[2] raw_light_level_pairs_and_packed_fields;

    public property float3 pos {
        get {
            return float3(
                raw_pos[0],
                raw_pos[1],
                raw_pos[2],
            );
        }
    }

    public property float4 tint_colour {
        get {
            return unpackUnorm4x8ToFloat(raw_tint_colour);
        }
    }

    public property uint4 light_level_pairs_1 {
        get {
            return uint4(
                raw_light_level_pairs_and_packed_fields[0] & 0xFF,
                (raw_light_level_pairs_and_packed_fields[0] >> 8) & 0xFF,
                (raw_light_level_pairs_and_packed_fields[0] >> 16) & 0xFF,
                (raw_light_level_pairs_and_packed_fields[0] >> 24) & 0xFF,
            );
        }
    }

    public property uint4 light_level_pairs_2_and_packed_fields {
        get {
            return uint4(
                raw_light_level_pairs_and_packed_fields[1] & 0xFF,
                (raw_light_level_pairs_and_packed_fields[1] >> 8) & 0xFF,
                (raw_light_level_pairs_and_packed_fields[1] >> 16) & 0xFF,
                (raw_light_level_pairs_and_packed_fields[1] >> 24) & 0xFF,
            );
        }
    }
}

// Bindings

[[vk::binding(0)]]
public ConstantBuffer<RenderInfo, Std430DataLayout> render_info;

[[vk::binding(1)]]
public Buffer<float4> lightmap;

[[vk::binding(2)]]
public Sampler2D block_item_atlas;

[[vk::binding(3)]]
public StructuredBuffer<CustomBlockVertex[4], Std430DataLayout> custom_block_faces;

[[vk::binding(4)]]
public Sampler2D sun_image;

[[vk::binding(5)]]
public Sampler2D moon_phases_image;
