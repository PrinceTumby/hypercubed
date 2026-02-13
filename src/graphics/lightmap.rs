use nalgebra::Vector3;

pub type RawLightmapTexture = [[[u8; 4]; 16]; 16];

pub const fn generate_dummy_lightmap_texture() -> RawLightmapTexture {
    [[[0x00, 0x00, 0x00, 0xFF]; 16]; 16]
}

/// Returns the RGBA8 16x16 texture data for the lightmap.
/// `brightness` must be between 0.0 and 1.0.
pub fn generate_lightmap_texture(brightness: f32, time_of_day: f64) -> RawLightmapTexture {
    assert!((0.0..=1.0).contains(&brightness));
    // TODO: Pull from "overworld.json"
    let ambient_light_factor = 0.0;
    let (sky_factor, sky_light_colour) = {
        const DAY_SKY_LIGHT_COLOUR: Vector3<f32> = Vector3::new(1.0, 1.0, 1.0);
        const NIGHT_SKY_LIGHT_COLOUR: Vector3<f32> = Vector3::new(0.48, 0.48, 1.0);
        let day_cycle_tick = time_of_day.round() as u64 % 24000;
        let night_percentage = match day_cycle_tick {
            // Daytime.
            730..11270 => 0.0,
            // Turning night.
            11270..13140 => (day_cycle_tick - 11270) as f32 / (13140 - 11270) as f32,
            // Night.
            13140..22860 => 1.0,
            // Turning day.
            22860..24000 | 0..730 => {
                let adjusted_tick = if (0_u64..730).contains(&day_cycle_tick) {
                    day_cycle_tick + 24000
                } else {
                    day_cycle_tick
                };
                1.0 - ((adjusted_tick - 22860) as f32 / (24730 - 22860) as f32)
            }
            _ => unreachable!(),
        };
        // let sky_factor = 1.0 - (night_percentage * 0.76);
        let sky_factor = 1.0 - (night_percentage * 0.90);
        let sky_light_colour = DAY_SKY_LIGHT_COLOUR.lerp(&NIGHT_SKY_LIGHT_COLOUR, night_percentage);
        (sky_factor, sky_light_colour)
    };
    // TODO: Add random flicker.
    // - Maybe just overlay a few sine waves to look vaguely random?
    // - Means we can run every frame, instead of once per tick.
    let block_factor = 1.5;
    // TODO:
    let night_vision_factor = 0.0;
    // TODO: Add a slider for this, once we've got a settings menu.
    let darkness_effect_scale = 0.0;
    // TODO:
    let darkness_gamma = 0.0;
    let darkness_scale = calculate_darkness_scale() * darkness_effect_scale;
    // TODO:
    let darken_world_factor = 0.0;
    let brightness_factor = (brightness - darkness_gamma).max(0.0);
    // TODO:
    let ambient_colour = Vector3::repeat(1.0);
    generate_lightmap_texture_inner(&GenerateLightmapTextureArgs {
        ambient_light_factor,
        sky_factor,
        block_factor,
        night_vision_factor,
        darkness_scale,
        darken_world_factor,
        brightness_factor,
        sky_light_colour,
        ambient_colour,
    })
}

#[derive(Clone, Copy, Debug)]
struct GenerateLightmapTextureArgs {
    pub ambient_light_factor: f32,
    pub sky_factor: f32,
    pub block_factor: f32,
    pub night_vision_factor: f32,
    pub darkness_scale: f32,
    pub darken_world_factor: f32,
    pub brightness_factor: f32,
    pub sky_light_colour: Vector3<f32>,
    pub ambient_colour: Vector3<f32>,
}

fn generate_lightmap_texture_inner(args: &GenerateLightmapTextureArgs) -> RawLightmapTexture {
    let GenerateLightmapTextureArgs {
        ambient_light_factor,
        sky_factor,
        block_factor,
        night_vision_factor,
        darkness_scale,
        darken_world_factor,
        brightness_factor,
        sky_light_colour,
        ambient_colour,
    } = *args;
    let mut out = [[[0x00; 4]; 16]; 16];
    for (y, out_row) in out.iter_mut().enumerate() {
        for (x, out_pixel) in out_row.iter_mut().enumerate() {
            fn get_brightness(level: f32) -> f32 {
                level / (4.0 - (3.0 * level))
            }
            let block_brightness = get_brightness(x as f32 / 15.0) * block_factor;
            let sky_brightness = get_brightness(y as f32 / 15.0) * sky_factor;
            let mut colour = Vector3::new(
                block_brightness * (block_brightness * 0.35 + 0.65),
                block_brightness * (block_brightness * 0.5 + 0.5),
                block_brightness * (block_brightness * 0.8 + 0.2),
            );
            colour = colour.lerp(&ambient_colour, ambient_light_factor);
            colour += sky_light_colour * sky_brightness;
            colour = colour.lerp(&Vector3::repeat(0.75), 0.04);
            if ambient_light_factor == 0.0 {
                let darkened_colour = colour.component_mul(&Vector3::new(0.7, 0.6, 0.6));
                colour = colour.lerp(&darkened_colour, darken_world_factor);
            }
            if night_vision_factor > 0.0 {
                let max_component = colour.max();
                if max_component < 1.0 {
                    let bright_colour = colour / max_component;
                    colour = colour.lerp(&bright_colour, night_vision_factor);
                }
            }
            if ambient_light_factor == 0.0 {
                colour -= Vector3::repeat(darkness_scale);
            }
            colour = colour.map(|n| n.clamp(0.0, 1.0));
            let fake_gamma: Vector3<f32> = {
                let max_component = colour.max();
                let max_inverted = 1.0 - max_component;
                let max_scaled = 1.0 - max_inverted.powi(4);
                colour * (max_scaled / max_component)
            };
            colour = colour.lerp(&fake_gamma, brightness_factor);
            colour = colour.lerp(&Vector3::repeat(0.75), 0.04);
            *out_pixel = [
                (colour.x * 255.0).round() as u8,
                (colour.y * 255.0).round() as u8,
                (colour.z * 255.0).round() as u8,
                0xFF,
            ];
        }
    }
    out
}

fn calculate_darkness_scale() -> f32 {
    // TODO:
    0.0
}
