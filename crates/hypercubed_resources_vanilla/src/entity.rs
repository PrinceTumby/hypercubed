use hypercubed_core::types::PercentageF32;
use resources::entity::Data as EntityData;
use resources::identifier_part;
use resources::identifier::{Identifier, IdentifierPart};

pub fn load_data() -> anyhow::Result<EntityData> {
    let mut atlas_builder = resources::texture::AtlasBuilder::new(
        [128; 2],
        resources::texture::AtlasAllocatorOptions {
            small_size_threshold: 16,
            large_size_threshold: 16,
            ..Default::default()
        },
    );
    todo!()
}

fn load_boat_variant_textures(
    atlas_builder: &mut resources::texture::AtlasBuilder,
    variant_name: &IdentifierPart,
) -> anyhow::Result<()> {
    macro_rules! texture_parts {
        (
            [$reference_width:literal, $reference_height:literal],
            [
                $((
                    $identifier_format:expr,
                    [$start_x:literal, $start_y:literal, $width:literal, $height:literal $(,)?]
                    $(,)?
                )),*
                $(,)?
            ]
            $(,)?
        ) => {
            [
                $((
                    Identifier::parse(&format!($identifier_format)).unwrap(),
                    [
                        PercentageF32::from_f32_0_1_clamped(
                            $start_x as f32 / $reference_width as f32
                        ),
                        PercentageF32::from_f32_0_1_clamped(
                            $start_y as f32 / $reference_height as f32
                        ),
                        PercentageF32::from_f32_0_1_clamped(
                            ($start_x + $width) as f32 / $reference_width as f32
                        ),
                        PercentageF32::from_f32_0_1_clamped(
                            ($start_y + $height) as f32 / $reference_height as f32
                        ),
                    ],
                )),*
            ]
        };
    }
    atlas_builder.load_texture_parts(
        &Identifier::parse(&format!("minecraft:entity/boat/{variant_name}")).unwrap(),
        texture_parts!(
            [128, 64],
            [
                // Bottom.
                ("hypercubed_vanilla:entity/boat/{variant_name}/bottom/down", [3, 3, 28, 16]),
                ("hypercubed_vanilla:entity/boat/{variant_name}/bottom/up", [34, 3, 28, 16]),
                ("hypercubed_vanilla:entity/boat/{variant_name}/bottom/south", [0, 3, 3, 16]),
                ("hypercubed_vanilla:entity/boat/{variant_name}/bottom/north", [31, 3, 3, 16]),
                ("hypercubed_vanilla:entity/boat/{variant_name}/bottom/east", [3, 0, 28, 3]),
                ("hypercubed_vanilla:entity/boat/{variant_name}/bottom/west", [31, 0, 28, 3]),
                // Back.
                ("hypercubed_vanilla:entity/boat/{variant_name}/back/down", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}/back/up", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}/back/south", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}/back/north", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}/back/east", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}/back/west", []),
                // Left.
                ("hypercubed_vanilla:entity/boat/{variant_name}//down", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//up", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//south", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//north", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//east", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//west", []),
                // Right.
                ("hypercubed_vanilla:entity/boat/{variant_name}//down", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//up", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//south", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//north", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//east", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//west", []),
                // Front.
                ("hypercubed_vanilla:entity/boat/{variant_name}//down", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//up", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//south", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//north", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//east", []),
                ("hypercubed_vanilla:entity/boat/{variant_name}//west", []),
            ],
        ),
    )
}
