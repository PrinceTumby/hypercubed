# Basic types
let UnsignedInteger =
  std.contract.from_predicate (fun value =>
    std.is_number value
    && value
    % 1 == 0
    && value >= 0
  )
in

let BoundedInteger = fun start end =>
  std.contract.from_predicate (fun value =>
    std.is_number value
    && value >= start
    && value <= end
  )
in

let Vec3 =
  std.contract.custom (fun label value =>
    std.contract.check (Array Number) label value
    |> match {
      'Error err_info => 'Error err_info,
      'Ok _value =>
        if std.array.length value != 3 then
          'Error { message = "array does not contain 3 elements" }
        else
          'Ok value
    }
  )
in

let LightLevel = BoundedInteger 0 15 in

let make_kv_list
  : forall a. { _ : a } -> Array { key : String, value : a }
  = fun record =>
    std.record.to_array record
    |> std.array.map (fun { field, value } => { key = field, include value })
in

# Common properties
let CustomPropertyType =
  std.contract.any_of [
    { Boolean | {} },
    { Int | { start | UnsignedInteger, end | Number } },
    { Enum | Array String },
  ]
in

let CustomProperty = { name | String, prop_type | CustomPropertyType } in

let bool_prop
  : String -> CustomProperty
  = fun name =>
    { include name, prop_type = { Boolean = {} } }
      | CustomProperty
in

let int_prop
  : String -> UnsignedInteger -> UnsignedInteger -> CustomProperty
  = fun name start end =>
    { include name, prop_type = { Int = { include start, include end } } }
      | CustomProperty
in

let enum_prop
  : String -> Array String -> CustomProperty
  = fun name variants =>
    { include name, prop_type = { Enum = variants } }
      | CustomProperty
in

let facing_nswe = enum_prop "facing" ["north", "south", "west", "east"] in

let facing_neswud = enum_prop "facing" ["north", "east", "south", "west", "up", "down"] in

let waterlogged = bool_prop "waterlogged" in

let powered = bool_prop "powered" in

let lit = bool_prop "lit" in

let stage_0_1 = int_prop "stage" 0 1 in

let age_0_15 = int_prop "age" 0 15 in

let age_0_25 = int_prop "age" 0 25 in

let rotation_0_15 = int_prop "rotation" 0 15 in

let chest_type = enum_prop "type" ["single", "left", "right"] in

let Properties = {
  air_like
    | Bool
    | default
    = false,
}
in

let BlockOpacity = [| 'Opaque, 'Leaves, 'Glass, 'GlassPane, 'Transparent |] in

let SkyLightOpacity = [| 'Opaque, 'Translucent, 'Transparent |] in

let AABB = { corner_1 | Array Number, corner_2 | Array Number } in

let make_aabb
  : Vec3 -> Vec3 -> AABB
  = fun start end =>
    { corner_1 = start, corner_2 = end }
      | AABB
in

let CollisionInfo =
  std.contract.any_of [
    { Empty | {} },
    { FullBlock | {} },
    { Complex | Array AABB },
  ]
in

let empty_collision_info = { Empty = {} } in

let complex_collision_info
  : Array AABB -> CollisionInfo
  = fun aabbs =>
    { Complex = aabbs }
      | CollisionInfo
in

let BlockstateInfo = {
  opacity
    | BlockOpacity
    | default
    = 'Opaque,
  light_info
    | {
      sky_light_opacity
        | SkyLightOpacity
        | default
        = 'Opaque,
      emission_level
        | LightLevel
        | default
        = 0,
    }
    | default
    = {},
  collision_info
    | CollisionInfo
    | default
    = { FullBlock = {} },
}
in

let sky_transparent_info = { sky_light_opacity = 'Transparent, emission_level = 0 } in

let BlockstateInfoModifier = {
  opacity
    | BlockOpacity
    | optional,
  light_info
    | {
      sky_light_opacity
        | SkyLightOpacity,
      emission_level
        | LightLevel,
    }
    | optional,
  collision_info
    | CollisionInfo
    | optional,
}
in

let BlockstateInfoModifierCase = {
  modifier
    | BlockstateInfoModifier,
  conditions
    | Array { key | String, value | String },
}
in

# Registrations
let StandardRegistration = {
  type
    | std.enum.Enum
    | default
    = 'Standard,
  identifier
    | String,
  custom_variants
    | Array CustomProperty
    | optional,
  replacement_variants
    | Array CustomProperty
    | optional,
  default_override
    | Array { key | String, value | String }
    | optional,
  properties
    | Properties
    | default
    = {},
  default_extra_info
    | BlockstateInfo
    | default
    = {},
  extra_info_modifiers
    | Array BlockstateInfoModifierCase
    | default
    = [],
}
in

let FullCustomRegistration = {
  type
    | std.enum.Enum
    | default
    = 'FullCustom,
  identifier
    | String,
  custom_variants
    | Array CustomProperty,
  skip_properties
    | Array String,
  default_override
    | Array { key | String, value | String }
    | optional,
  properties
    | Properties
    | default
    = {},
  default_extra_info
    | BlockstateInfo
    | default
    = {},
  extra_info_modifiers
    | Array BlockstateInfoModifierCase
    | default
    = [],
}
in

let LiquidRegistration = {
  type
    | std.enum.Enum
    | default
    = 'Liquid,
  identifier
    | String,
  properties
    | Properties
    | default
    = {},
  default_extra_info
    | BlockstateInfo
    | default
    = {},
  extra_info_modifiers
    | Array BlockstateInfoModifierCase
    | default
    = [],
}
in

let Registration =
  std.contract.any_of [
    LiquidRegistration,
    StandardRegistration,
    FullCustomRegistration,
  ]
in

let register_basic
  : String -> StandardRegistration
  = fun identifier =>
    { include identifier }
      | StandardRegistration
in

let register_basic_light
  : String -> LightLevel -> StandardRegistration
  = fun identifier emission_level =>
    {
      include identifier,
      default_extra_info = {
        light_info = {
          sky_light_opacity = 'Opaque,
          include emission_level,
        },
      },
    } | StandardRegistration
in

let register_basic_liquid
  : String -> LiquidRegistration
  = fun identifier =>
    {
      include identifier,
      default_extra_info = {
        opacity = 'Transparent,
        light_info = {
          sky_light_opacity = 'Translucent,
          emission_level = 0,
        },
      },
    } | LiquidRegistration
in

let register_basic_liquid_light
  : String -> LightLevel -> LiquidRegistration
  = fun identifier emission_level =>
    {
      include identifier,
      default_extra_info = {
        opacity = 'Transparent,
        light_info = {
          sky_light_opacity = 'Translucent,
          include emission_level,
        },
      },
    } | LiquidRegistration
in

let register_transparent
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
    } | StandardRegistration
in

let register_transparent_light
  : String -> LightLevel -> StandardRegistration
  = fun identifier emission_level =>
    {
      include identifier,
      default_extra_info = {
        opacity = 'Transparent,
        light_info = {
          sky_light_opacity = 'Transparent,
          include emission_level,
        },
      },
    } | StandardRegistration
in

let register_transparent_no_collider
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
        collision_info = empty_collision_info,
      },
    } | StandardRegistration
in

let register_transparent_light_no_collider
  : String -> LightLevel -> StandardRegistration
  = fun identifier emission_level =>
    {
      include identifier,
      default_extra_info = {
        opacity = 'Transparent,
        light_info = {
          sky_light_opacity = 'Transparent,
          include emission_level,
        },
        collision_info = empty_collision_info,
      },
    } | StandardRegistration
in

let register_log
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      default_override = make_kv_list { axis = "y" },
    } | StandardRegistration
in

let register_leaves
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [
        int_prop "distance" 1 7,
        bool_prop "persistent",
        waterlogged,
      ],
      default_override =
        make_kv_list {
          distance = "7",
          persistent = "false",
          waterlogged = "false",
        },
      default_extra_info = {
        opacity = 'Leaves,
        light_info = {
          sky_light_opacity = 'Translucent,
          emission_level = 0,
        },
      },
    } | StandardRegistration
in

let register_bed
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      default_override = make_kv_list { facing = "north", occupied = "false", part = "foot" },
      default_extra_info = {
        opacity = 'Transparent,
        collision_info =
          complex_collision_info [
            make_aabb [0.0, 0.0, 0.0] [1.0, 0.5625, 1.0]
          ],
      },
    } | StandardRegistration
in

let register_slab
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [waterlogged],
      replacement_variants = [enum_prop "type" ["top", "bottom", "double"]],
      default_override = make_kv_list { type = "bottom", waterlogged = "false" },
      default_extra_info = { opacity = 'Transparent },
      extra_info_modifiers = [
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.0] [1.0, 0.5, 1.0]
              ],
          },
          conditions = make_kv_list { type = "bottom" },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.5, 0.0] [1.0, 1.0, 1.0]
              ],
          },
          conditions = make_kv_list { type = "top" },
        },
      ],
    } | StandardRegistration
in

let register_stairs
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [waterlogged],
      replacement_variants = [
        facing_nswe,
        enum_prop "half" ["top", "bottom"],
        enum_prop
          "shape"
          [
            "straight",
            "inner_left",
            "inner_right",
            "outer_left",
            "outer_right",
          ],
      ],
      default_override =
        make_kv_list {
          facing = "north",
          half = "bottom",
          shape = "straight",
          waterlogged = "false",
        },
      default_extra_info = { opacity = 'Transparent },
      extra_info_modifiers = [
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.0] [1.0, 0.5, 1.0],
                make_aabb [0.5, 0.5, 0.0] [1.0, 1.0, 1.0],
              ],
          },
          conditions = make_kv_list { shape = "straight" },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.0] [1.0, 0.5, 1.0],
                make_aabb [0.5, 0.5, 0.0] [1.0, 1.0, 1.0],
                make_aabb [0.0, 0.5, 0.0] [0.5, 1.0, 0.5],
              ],
          },
          conditions = make_kv_list { shape = "inner_left" },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.0] [1.0, 0.5, 1.0],
                make_aabb [0.5, 0.5, 0.0] [1.0, 1.0, 1.0],
                make_aabb [0.0, 0.5, 0.5] [0.5, 1.0, 1.0],
              ],
          },
          conditions = make_kv_list { shape = "inner_right" },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.0] [1.0, 0.5, 1.0],
                make_aabb [0.5, 0.5, 0.5] [1.0, 1.0, 1.0],
              ],
          },
          conditions = make_kv_list { shape = "outer_left" },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.0] [1.0, 0.5, 1.0],
                make_aabb [0.5, 0.5, 0.5] [1.0, 1.0, 1.0],
              ],
          },
          conditions = make_kv_list { shape = "outer_right" },
        },
      ],
    } | StandardRegistration
in

let register_fence
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [
        bool_prop "east",
        bool_prop "north",
        bool_prop "south",
        waterlogged,
        bool_prop "west",
      ],
      default_override =
        make_kv_list {
          east = "false",
          north = "false",
          south = "false",
          waterlogged = "false",
          west = "false",
        },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
      extra_info_modifiers = [
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.375, 0.0, 0.375] [0.625, 1.5, 0.625],
              ],
          },
          conditions =
            make_kv_list {
              east = "false",
              north = "false",
              south = "false",
              west = "false",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.375, 0.0, 0.375] [1.0, 1.5, 0.625],
              ],
          },
          conditions =
            make_kv_list {
              east = "true",
              north = "false",
              south = "false",
              west = "false",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.375, 0.0, 0.0] [0.625, 1.5, 0.625],
              ],
          },
          conditions =
            make_kv_list {
              east = "false",
              north = "true",
              south = "false",
              west = "false",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.375, 0.0, 0.375] [1.0, 1.5, 0.625],
                make_aabb [0.375, 0.0, 0.0] [0.625, 1.5, 0.375],
              ],
          },
          conditions =
            make_kv_list {
              east = "true",
              north = "true",
              south = "false",
              west = "false",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.375, 0.0, 0.375] [0.625, 1.5, 1.0],
              ],
          },
          conditions =
            make_kv_list {
              east = "false",
              north = "false",
              south = "true",
              west = "false",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.375, 0.0, 0.375] [1.0, 1.5, 1.0],
                make_aabb [0.375, 0.0, 0.625] [1.0, 1.5, 1.0],
              ],
          },
          conditions =
            make_kv_list {
              east = "true",
              north = "false",
              south = "true",
              west = "false",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.375, 0.0, 0.0] [0.625, 1.5, 1.0],
              ],
          },
          conditions =
            make_kv_list {
              east = "false",
              north = "true",
              south = "true",
              west = "false",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.375, 0.0, 0.375] [1.0, 1.5, 0.625],
                make_aabb [0.375, 0.0, 0.0] [0.625, 1.5, 1.0],
              ],
          },
          conditions =
            make_kv_list {
              east = "true",
              north = "true",
              south = "true",
              west = "false",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.375] [0.625, 1.5, 0.625],
              ],
          },
          conditions =
            make_kv_list {
              east = "false",
              north = "false",
              south = "false",
              west = "true",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.375] [1.0, 1.5, 0.625],
              ],
          },
          conditions =
            make_kv_list {
              east = "true",
              north = "false",
              south = "false",
              west = "true",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.375] [0.375, 1.5, 0.625],
                make_aabb [0.375, 0.0, 0.0] [0.625, 1.5, 0.625],
              ],
          },
          conditions =
            make_kv_list {
              east = "false",
              north = "true",
              south = "false",
              west = "true",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.375] [1.0, 1.5, 0.625],
                make_aabb [0.375, 0.0, 0.0] [0.625, 1.5, 0.375],
              ],
          },
          conditions =
            make_kv_list {
              east = "true",
              north = "true",
              south = "false",
              west = "true",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.375] [0.375, 1.5, 0.625],
                make_aabb [0.375, 0.0, 0.375] [0.625, 1.5, 1.0],
              ],
          },
          conditions =
            make_kv_list {
              east = "false",
              north = "false",
              south = "true",
              west = "true",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.375] [1.0, 1.5, 0.625],
                make_aabb [0.375, 0.0, 0.625] [1.0, 1.5, 1.0],
              ],
          },
          conditions =
            make_kv_list {
              east = "true",
              north = "false",
              south = "true",
              west = "true",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.375] [0.625, 1.5, 0.625],
                make_aabb [0.375, 0.0, 0.0] [0.625, 1.5, 1.0],
              ],
          },
          conditions =
            make_kv_list {
              east = "false",
              north = "true",
              south = "true",
              west = "true",
            },
        },
        {
          modifier = {
            collision_info =
              complex_collision_info [
                make_aabb [0.0, 0.0, 0.375] [1.0, 1.5, 0.625],
                make_aabb [0.375, 0.0, 0.0] [0.625, 1.5, 1.0],
              ],
          },
          conditions =
            make_kv_list {
              east = "true",
              north = "true",
              south = "true",
              west = "true",
            },
        },
      ]
    } | StandardRegistration
in

let register_fence_gate
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [powered],
      replacement_variants = [facing_nswe, bool_prop "in_wall", bool_prop "open"],
      default_override =
        make_kv_list {
          facing = "north",
          in_wall = "false",
          open = "false",
          powered = "false",
        },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
    } | StandardRegistration
in

let register_wall
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [
        enum_prop "east" ["none", "low", "tall"],
        enum_prop "north" ["none", "low", "tall"],
        enum_prop "south" ["none", "low", "tall"],
        bool_prop "up",
        waterlogged,
        enum_prop "west" ["none", "low", "tall"],
      ],
      default_override =
        make_kv_list {
          east = "none",
          north = "none",
          south = "none",
          up = "true",
          waterlogged = "false",
          west = "none",
        },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
    } | StandardRegistration
in

let register_door
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [powered],
      replacement_variants = [
        facing_nswe,
        enum_prop "half" ["upper", "lower"],
        enum_prop "hinge" ["left", "right"],
        bool_prop "open",
      ],
      default_override =
        make_kv_list {
          facing = "north",
          half = "lower",
          hinge = "left",
          open = "false",
          powered = "false",
        },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
    } | StandardRegistration
in

let register_trapdoor
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [powered, waterlogged],
      replacement_variants = [
        facing_nswe,
        enum_prop "half" ["top", "bottom"],
        bool_prop "open",
      ],
      default_override =
        make_kv_list {
          facing = "north",
          half = "bottom",
          open = "false",
          powered = "false",
          waterlogged = "false",
        },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
    } | StandardRegistration
in

let register_chest
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [facing_nswe, chest_type, waterlogged],
      default_override =
        make_kv_list {
          facing = "north",
          type = "single",
          waterlogged = "false",
        },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
    } | StandardRegistration
in

let register_sign
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [rotation_0_15, waterlogged],
      default_override =
        make_kv_list {
          rotation = "0",
          waterlogged = "false",
        },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
        collision_info = empty_collision_info,
      },
    } | StandardRegistration
in

let register_wall_sign
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [facing_nswe, waterlogged],
      default_override =
        make_kv_list {
          facing = "north",
          waterlogged = "false",
        },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
        collision_info = empty_collision_info,
      },
    } | StandardRegistration
in

let register_hanging_sign
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [bool_prop "attached", rotation_0_15, waterlogged],
      default_override =
        make_kv_list {
          attached = "false",
          rotation = "0",
          waterlogged = "false",
        },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
        collision_info = empty_collision_info,
      },
    } | StandardRegistration
in

let register_wall_hanging_sign
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [facing_nswe, waterlogged],
      default_override =
        make_kv_list {
          facing = "north",
          waterlogged = "false",
        },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
        collision_info = empty_collision_info,
      },
    } | StandardRegistration
in

let register_stained_glass
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
    } | StandardRegistration
in

let register_grate
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [waterlogged],
      default_override = make_kv_list { waterlogged = "false" },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
    } | StandardRegistration
in

let register_pressure_plate
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      replacement_variants = [powered],
      default_override = make_kv_list { powered = "false" },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
        collision_info = empty_collision_info,
      },
    } | StandardRegistration
in

let register_button
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      replacement_variants = [
        enum_prop "face" ["floor", "wall", "ceiling"],
        facing_nswe,
        powered,
      ],
      default_override =
        make_kv_list {
          face = "wall",
          facing = "north",
          powered = "false",
        },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
        collision_info = empty_collision_info,
      },
    } | StandardRegistration
in

let register_mushroom_block
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [
        bool_prop "down",
        bool_prop "east",
        bool_prop "north",
        bool_prop "south",
        bool_prop "up",
        bool_prop "west",
      ],
    } | StandardRegistration
in

let register_head
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [powered, rotation_0_15],
      default_override = make_kv_list { powered = "false", rotation = "0" },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
    } | StandardRegistration
in

let register_wall_head
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [facing_nswe, powered],
      default_override = make_kv_list { facing = "north", powered = "false" },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
    } | StandardRegistration
in

let register_weighted_pressure_plate
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      replacement_variants = [int_prop "power" 0 15],
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
        collision_info = empty_collision_info,
      },
    } | StandardRegistration
in

let register_glass_pane
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [
        bool_prop "east",
        bool_prop "north",
        bool_prop "south",
        waterlogged,
        bool_prop "west",
      ],
      default_override =
        make_kv_list {
          east = "false",
          north = "false",
          south = "false",
          waterlogged = "false",
          west = "false",
        },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
    } | StandardRegistration
in

let register_banner
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [rotation_0_15],
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
        collision_info = empty_collision_info,
      },
    } | StandardRegistration
in

let register_wall_banner
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [facing_nswe],
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
        collision_info = empty_collision_info,
      },
    } | StandardRegistration
in

let register_shulker_box
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [facing_neswud],
      default_override = make_kv_list { facing = "up" },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
    } | StandardRegistration
in

let register_glazed_terracotta
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      replacement_variants = [facing_nswe],
    } | StandardRegistration
in

let register_coral
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [waterlogged],
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
        collision_info = empty_collision_info,
      },
    } | StandardRegistration
in

let register_coral_wall_fan
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      custom_variants = [waterlogged],
      replacement_variants = [facing_nswe],
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
        collision_info = empty_collision_info,
      },
    } | StandardRegistration
in

let register_lantern
  : String -> LightLevel -> StandardRegistration
  = fun identifier emission_level =>
    {
      include identifier,
      custom_variants = [waterlogged],
      replacement_variants = [bool_prop "hanging"],
      default_override = make_kv_list { hanging = "false", waterlogged = "false" },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = {
          sky_light_opacity = 'Transparent,
          include emission_level,
        },
      },
    } | StandardRegistration
in

let register_candle
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      replacement_variants = [int_prop "candles" 1 4, lit],
      custom_variants = [waterlogged],
      default_override = make_kv_list { candles = "1", lit = "false", waterlogged = "false" },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
      extra_info_modifiers = [
        {
          modifier = {
            light_info = {
              sky_light_opacity = 'Transparent,
              emission_level = 12,
            },
          },
          conditions = make_kv_list { candles = "4", lit = "true" },
        },
        {
          modifier = {
            light_info = {
              sky_light_opacity = 'Transparent,
              emission_level = 9,
            },
          },
          conditions = make_kv_list { candles = "3", lit = "true" },
        },
        {
          modifier = {
            light_info = {
              sky_light_opacity = 'Transparent,
              emission_level = 6,
            },
          },
          conditions = make_kv_list { candles = "2", lit = "true" },
        },
        {
          modifier = {
            light_info = {
              sky_light_opacity = 'Transparent,
              emission_level = 3,
            },
          },
          conditions = make_kv_list { candles = "1", lit = "true" },
        },
      ],
    } | StandardRegistration
in

let register_candle_cake
  : String -> StandardRegistration
  = fun identifier =>
    {
      include identifier,
      replacement_variants = [lit],
      default_override = make_kv_list { lit = "false" },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = sky_transparent_info,
      },
    } | StandardRegistration
in

let register_amethyst_bud
  : String -> LightLevel -> StandardRegistration
  = fun identifier emission_level =>
    {
      include identifier,
      custom_variants = [waterlogged],
      replacement_variants = [facing_neswud],
      default_override = make_kv_list { facing = "up", waterlogged = "false" },
      default_extra_info = {
        opacity = 'Transparent,
        light_info = {
          sky_light_opacity = 'Transparent,
          include emission_level,
        },
      },
    } | StandardRegistration
in

let register_bulb
  : String -> LightLevel -> StandardRegistration
  = fun identifier emission_level =>
    {
      include identifier,
      replacement_variants = [lit, powered],
      default_override = make_kv_list { lit = "false", powered = "false" },
      extra_info_modifiers = [
        {
          modifier = {
            light_info = {
              sky_light_opacity = 'Opaque,
              include emission_level,
            },
          },
          conditions = make_kv_list { lit = "true" },
        },
      ],
    } | StandardRegistration
in

let items | Array Registration = [
  {
    identifier = "air",
    properties = { air_like = true },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 0,
      },
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  register_basic "stone",
  register_basic "granite",
  register_basic "polished_granite",
  register_basic "diorite",
  register_basic "polished_diorite",
  register_basic "andesite",
  register_basic "polished_andesite",
  {
    identifier = "grass_block",
    default_override = make_kv_list { snowy = "false" },
    replacement_variants = [bool_prop "snowy"],
  } | StandardRegistration,
  register_basic "dirt",
  register_basic "coarse_dirt",
  {
    identifier = "podzol",
    default_override = make_kv_list { snowy = "false" },
    replacement_variants = [bool_prop "snowy"],
  } | StandardRegistration,
  register_basic "cobblestone",
  register_basic "oak_planks",
  register_basic "spruce_planks",
  register_basic "birch_planks",
  register_basic "jungle_planks",
  register_basic "acacia_planks",
  register_basic "cherry_planks",
  register_basic "dark_oak_planks",
  register_basic "mangrove_planks",
  register_basic "bamboo_planks",
  register_basic "bamboo_mosaic",
  {
    identifier = "oak_sapling",
    custom_variants = [stage_0_1],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    }
  } | StandardRegistration,
  {
    identifier = "spruce_sapling",
    custom_variants = [stage_0_1],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    }
  } | StandardRegistration,
  {
    identifier = "birch_sapling",
    custom_variants = [stage_0_1],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    }
  } | StandardRegistration,
  {
    identifier = "jungle_sapling",
    custom_variants = [stage_0_1],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    }
  } | StandardRegistration,
  {
    identifier = "acacia_sapling",
    custom_variants = [stage_0_1],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    }
  } | StandardRegistration,
  {
    identifier = "cherry_sapling",
    custom_variants = [stage_0_1],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    }
  } | StandardRegistration,
  {
    identifier = "dark_oak_sapling",
    custom_variants = [stage_0_1],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    }
  } | StandardRegistration,
  {
    identifier = "mangrove_propagule",
    custom_variants = [stage_0_1, waterlogged],
    replacement_variants = [int_prop "age" 0 4, bool_prop "hanging"],
    default_override =
      make_kv_list {
        age = "0",
        hanging = "false",
        stage = "0",
        waterlogged = "false",
      },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    }
  } | StandardRegistration,
  register_basic "bedrock",
  register_basic_liquid "water",
  register_basic_liquid_light "lava" 15,
  register_basic "sand",
  register_basic "suspicious_sand",
  register_basic "red_sand",
  register_basic "gravel",
  register_basic "suspicious_gravel",
  register_basic "gold_ore",
  register_basic "deepslate_gold_ore",
  register_basic "iron_ore",
  register_basic "deepslate_iron_ore",
  register_basic "coal_ore",
  register_basic "deepslate_coal_ore",
  register_basic "nether_gold_ore",
  register_log "oak_log",
  register_log "spruce_log",
  register_log "birch_log",
  register_log "jungle_log",
  register_log "acacia_log",
  register_log "cherry_log",
  register_log "dark_oak_log",
  register_log "mangrove_log",
  {
    identifier = "mangrove_roots",
    custom_variants = [waterlogged],
    default_override = make_kv_list { waterlogged = "false" },
  } | StandardRegistration,
  register_log "muddy_mangrove_roots",
  register_log "bamboo_block",
  register_log "stripped_spruce_log",
  register_log "stripped_birch_log",
  register_log "stripped_jungle_log",
  register_log "stripped_acacia_log",
  register_log "stripped_cherry_log",
  register_log "stripped_dark_oak_log",
  register_log "stripped_oak_log",
  register_log "stripped_mangrove_log",
  register_log "stripped_bamboo_block",
  register_log "oak_wood",
  register_log "spruce_wood",
  register_log "birch_wood",
  register_log "jungle_wood",
  register_log "acacia_wood",
  register_log "cherry_wood",
  register_log "dark_oak_wood",
  register_log "mangrove_wood",
  register_log "stripped_oak_wood",
  register_log "stripped_spruce_wood",
  register_log "stripped_birch_wood",
  register_log "stripped_jungle_wood",
  register_log "stripped_acacia_wood",
  register_log "stripped_cherry_wood",
  register_log "stripped_dark_oak_wood",
  register_log "stripped_mangrove_wood",
  register_leaves "oak_leaves",
  register_leaves "spruce_leaves",
  register_leaves "birch_leaves",
  register_leaves "jungle_leaves",
  register_leaves "acacia_leaves",
  register_leaves "cherry_leaves",
  register_leaves "dark_oak_leaves",
  register_leaves "mangrove_leaves",
  register_leaves "azalea_leaves",
  register_leaves "flowering_azalea_leaves",
  register_basic "sponge",
  register_basic "wet_sponge",
  {
    identifier = "glass",
    default_extra_info = { opacity = 'Glass }
  } | StandardRegistration,
  register_basic "lapis_ore",
  register_basic "deepslate_lapis_ore",
  register_basic "lapis_block",
  {
    identifier = "dispenser",
    custom_variants = [bool_prop "triggered"],
    replacement_variants = [facing_neswud],
    default_override =
      make_kv_list { facing = "north", triggered = "false" },
  } | StandardRegistration,
  register_basic "sandstone",
  register_basic "chiseled_sandstone",
  register_basic "cut_sandstone",
  {
    identifier = "note_block",
    custom_variants = [
      enum_prop
        "instrument"
        [
          "harp",
          "basedrum",
          "snare",
          "hat",
          "bass",
          "flute",
          "bell",
          "guitar",
          "chime",
          "xylophone",
          "iron_xylophone",
          "cow_bell",
          "didgeridoo",
          "bit",
          "banjo",
          "pling",
          "zombie",
          "skeleton",
          "creeper",
          "dragon",
          "wither_skeleton",
          "piglin",
          "custom_head",
        ],
      int_prop "note" 0 24,
      powered,
    ],
    default_override =
      make_kv_list { instrument = "harp", note = "0", powered = "false" },
  } | StandardRegistration,
  register_bed "white_bed",
  register_bed "orange_bed",
  register_bed "magenta_bed",
  register_bed "light_blue_bed",
  register_bed "yellow_bed",
  register_bed "lime_bed",
  register_bed "pink_bed",
  register_bed "gray_bed",
  register_bed "light_gray_bed",
  register_bed "cyan_bed",
  register_bed "purple_bed",
  register_bed "blue_bed",
  register_bed "brown_bed",
  register_bed "green_bed",
  register_bed "red_bed",
  register_bed "black_bed",
  {
    identifier = "powered_rail",
    custom_variants = [waterlogged],
    replacement_variants = [
      powered,
      enum_prop
        "shape"
        [
          "north_south",
          "east_west",
          "ascending_east",
          "ascending_west",
          "ascending_north",
          "ascending_south",
        ],
    ],
    default_override =
      make_kv_list {
        powered = "false",
        shape = "north_south",
        waterlogged = "false",
      },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    }
  } | StandardRegistration,
  {
    identifier = "detector_rail",
    custom_variants = [waterlogged],
    replacement_variants = [
      powered,
      enum_prop
        "shape"
        [
          "north_south",
          "east_west",
          "ascending_east",
          "ascending_west",
          "ascending_north",
          "ascending_south",
        ],
    ],
    default_override =
      make_kv_list {
        powered = "false",
        shape = "north_south",
        waterlogged = "false",
      },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    }
  } | StandardRegistration,
  {
    identifier = "sticky_piston",
    replacement_variants = [bool_prop "extended", facing_neswud],
    default_override =
      make_kv_list { extended = "false", facing = "north" },
    extra_info_modifiers = [
      {
        modifier = {
          opacity = 'Transparent,
          light_info = sky_transparent_info,
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.25] [1.0, 1.0, 1.0],
            ],
        },
        conditions = make_kv_list { extended = "true" },
      }
    ],
  } | StandardRegistration,
  register_transparent_no_collider "cobweb",
  register_transparent_no_collider "short_grass",
  register_transparent_no_collider "fern",
  register_transparent_no_collider "dead_bush",
  register_transparent_no_collider "seagrass",
  {
    identifier = "tall_seagrass",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "piston",
    replacement_variants = [bool_prop "extended", facing_neswud],
    default_override =
      make_kv_list { extended = "false", facing = "north" },
    extra_info_modifiers = [
      {
        modifier = {
          opacity = 'Transparent,
          light_info = sky_transparent_info,
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.25] [1.0, 1.0, 1.0],
            ],
        },
        conditions = make_kv_list { extended = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "piston_head",
    replacement_variants = [
      facing_neswud,
      bool_prop "short",
      enum_prop "type" ["normal", "sticky"],
    ],
    default_override =
      make_kv_list { facing = "north", short = "false", type = "normal" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info =
        complex_collision_info [
          # Stem
          make_aabb [0.375, 0.375, 0.25] [0.625, 0.625, 1.0],
          # Head
          make_aabb [0.0, 0.0, 0.0] [1.0, 1.0, 0.25],
        ],
    },
  } | StandardRegistration,
  register_basic "white_wool",
  register_basic "orange_wool",
  register_basic "magenta_wool",
  register_basic "light_blue_wool",
  register_basic "yellow_wool",
  register_basic "lime_wool",
  register_basic "pink_wool",
  register_basic "gray_wool",
  register_basic "light_gray_wool",
  register_basic "cyan_wool",
  register_basic "purple_wool",
  register_basic "blue_wool",
  register_basic "brown_wool",
  register_basic "green_wool",
  register_basic "red_wool",
  register_basic "black_wool",
  {
    identifier = "moving_piston",
    custom_variants = [facing_neswud, enum_prop "type" ["normal", "sticky"]],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_transparent_no_collider "dandelion",
  register_transparent_no_collider "torchflower",
  register_transparent_no_collider "poppy",
  register_transparent_no_collider "blue_orchid",
  register_transparent_no_collider "allium",
  register_transparent_no_collider "azure_bluet",
  register_transparent_no_collider "red_tulip",
  register_transparent_no_collider "orange_tulip",
  register_transparent_no_collider "white_tulip",
  register_transparent_no_collider "pink_tulip",
  register_transparent_no_collider "oxeye_daisy",
  register_transparent_no_collider "cornflower",
  register_transparent_no_collider "wither_rose",
  register_transparent_no_collider "lily_of_the_valley",
  register_transparent_light "brown_mushroom" 1,
  register_transparent_no_collider "red_mushroom",
  register_basic "gold_block",
  register_basic "iron_block",
  register_basic "bricks",
  {
    identifier = "tnt",
    custom_variants = [bool_prop "unstable"],
    default_override = make_kv_list { unstable = "false" },
  } | StandardRegistration,
  register_basic "bookshelf",
  {
    identifier = "chiseled_bookshelf",
    custom_variants = [
      facing_nswe,
      bool_prop "slot_0_occupied",
      bool_prop "slot_1_occupied",
      bool_prop "slot_2_occupied",
      bool_prop "slot_3_occupied",
      bool_prop "slot_4_occupied",
      bool_prop "slot_5_occupied",
    ],
    default_override =
      make_kv_list {
        facing = "north",
        slot_0_occupied = "false",
        slot_1_occupied = "false",
        slot_2_occupied = "false",
        slot_3_occupied = "false",
        slot_4_occupied = "false",
        slot_5_occupied = "false",
      },
  } | StandardRegistration,
  register_basic "mossy_cobblestone",
  register_basic "obsidian",
  register_transparent_light_no_collider "torch" 14,
  {
    identifier = "wall_torch",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 14,
      },
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "fire",
    custom_variants = [
      age_0_15,
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      bool_prop "up",
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        age = "0",
        east = "false",
        north = "false",
        south = "false",
        up = "false",
        west = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 15,
      },
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "soul_fire",
    custom_variants = [],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 10,
      },
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  register_transparent "spawner",
  register_stairs "oak_stairs",
  register_chest "chest",
  {
    identifier = "redstone_wire",
    custom_variants = [
      enum_prop "east" ["up", "side", "none"],
      enum_prop "north" ["up", "side", "none"],
      int_prop "power" 0 15,
      enum_prop "south" ["up", "side", "none"],
      enum_prop "west" ["up", "side", "none"],
    ],
    default_override =
      make_kv_list {
        east = "none",
        north = "none",
        south = "none",
        west = "none",
        power = "0",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_basic "diamond_ore",
  register_basic "deepslate_diamond_ore",
  register_basic "diamond_block",
  register_basic "crafting_table",
  register_transparent_no_collider "wheat",
  register_transparent "farmland",
  {
    identifier = "furnace",
    replacement_variants = [facing_nswe, lit],
    default_override =
      make_kv_list { facing = "north", lit = "false" },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 13,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  register_sign "oak_sign",
  register_sign "spruce_sign",
  register_sign "birch_sign",
  register_sign "acacia_sign",
  register_sign "cherry_sign",
  register_sign "jungle_sign",
  register_sign "dark_oak_sign",
  register_sign "mangrove_sign",
  register_sign "bamboo_sign",
  register_door "oak_door",
  {
    identifier = "ladder",
    custom_variants = [waterlogged],
    replacement_variants = [facing_nswe],
    default_override =
      make_kv_list { facing = "north", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "rail",
    custom_variants = [waterlogged],
    replacement_variants = [
      enum_prop
        "shape"
        [
          "north_south",
          "east_west",
          "ascending_east",
          "ascending_west",
          "ascending_north",
          "ascending_south",
          "south_east",
          "south_west",
          "north_west",
          "north_east",
        ],
    ],
    default_override =
      make_kv_list { shape = "north_south", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_stairs "cobblestone_stairs",
  register_wall_sign "oak_wall_sign",
  register_wall_sign "spruce_wall_sign",
  register_wall_sign "birch_wall_sign",
  register_wall_sign "acacia_wall_sign",
  register_wall_sign "cherry_wall_sign",
  register_wall_sign "jungle_wall_sign",
  register_wall_sign "dark_oak_wall_sign",
  register_wall_sign "mangrove_wall_sign",
  register_wall_sign "bamboo_wall_sign",
  register_hanging_sign "oak_hanging_sign",
  register_hanging_sign "spruce_hanging_sign",
  register_hanging_sign "birch_hanging_sign",
  register_hanging_sign "acacia_hanging_sign",
  register_hanging_sign "cherry_hanging_sign",
  register_hanging_sign "jungle_hanging_sign",
  register_hanging_sign "dark_oak_hanging_sign",
  register_hanging_sign "crimson_hanging_sign",
  register_hanging_sign "warped_hanging_sign",
  register_hanging_sign "mangrove_hanging_sign",
  register_hanging_sign "bamboo_hanging_sign",
  register_wall_hanging_sign "oak_wall_hanging_sign",
  register_wall_hanging_sign "spruce_wall_hanging_sign",
  register_wall_hanging_sign "birch_wall_hanging_sign",
  register_wall_hanging_sign "acacia_wall_hanging_sign",
  register_wall_hanging_sign "cherry_wall_hanging_sign",
  register_wall_hanging_sign "jungle_wall_hanging_sign",
  register_wall_hanging_sign "dark_oak_wall_hanging_sign",
  register_wall_hanging_sign "mangrove_wall_hanging_sign",
  register_wall_hanging_sign "crimson_wall_hanging_sign",
  register_wall_hanging_sign "warped_wall_hanging_sign",
  register_wall_hanging_sign "bamboo_wall_hanging_sign",
  {
    identifier = "lever",
    replacement_variants = [
      enum_prop "face" ["floor", "wall", "ceiling"],
      facing_nswe,
      powered,
    ],
    default_override =
      make_kv_list { face = "wall", facing = "north", powered = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  register_pressure_plate "stone_pressure_plate",
  {
    identifier = "iron_door",
    custom_variants = [powered],
    replacement_variants = [
      facing_nswe,
      enum_prop "half" ["upper", "lower"],
      enum_prop "hinge" ["left", "right"],
      bool_prop "open",
    ],
    default_override =
      make_kv_list {
        facing = "north",
        half = "lower",
        hinge = "left",
        open = "false",
        powered = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_pressure_plate "oak_pressure_plate",
  register_pressure_plate "spruce_pressure_plate",
  register_pressure_plate "birch_pressure_plate",
  register_pressure_plate "jungle_pressure_plate",
  register_pressure_plate "acacia_pressure_plate",
  register_pressure_plate "cherry_pressure_plate",
  register_pressure_plate "dark_oak_pressure_plate",
  register_pressure_plate "mangrove_pressure_plate",
  register_pressure_plate "bamboo_pressure_plate",
  {
    identifier = "redstone_ore",
    custom_variants = [lit],
    default_override = make_kv_list { lit = "false" },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "deepslate_redstone_ore",
    custom_variants = [lit],
    default_override = make_kv_list { lit = "false" },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "redstone_torch",
    replacement_variants = [lit],
    default_extra_info = {
      opacity = 'Transparent,
      collision_info = empty_collision_info,
    },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 7
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "redstone_wall_torch",
    replacement_variants = [facing_nswe, lit],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Opaque,
        emission_level = 7
      },
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  register_button "stone_button",
  {
    identifier = "snow",
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
    extra_info_modifiers = [
      {
        modifier = {
          collision_info = empty_collision_info,
        },
        conditions = make_kv_list { layers = "1" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.125, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "2" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.25, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "3" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.375, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "4" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.5, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "5" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.625, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "6" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.75, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "7" },
      },
      {
        modifier = {
          collision_info =
            complex_collision_info [
              make_aabb [0.0, 0.0, 0.0] [1.0, 0.875, 1.0],
            ],
        },
        conditions = make_kv_list { layers = "8" },
      },
    ],
  } | StandardRegistration,
  register_transparent "ice",
  register_basic "snow_block",
  {
    identifier = "cactus",
    custom_variants = [age_0_15],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_basic "clay",
  {
    identifier = "sugar_cane",
    custom_variants = [age_0_15],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "jukebox",
    custom_variants = [bool_prop "has_record"],
    default_override = make_kv_list { has_record = "false" },
  } | StandardRegistration,
  register_fence "oak_fence",
  register_basic "netherrack",
  register_transparent "soul_sand",
  register_basic "soul_soil",
  {
    identifier = "basalt",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "polished_basalt",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  register_transparent_light_no_collider "soul_torch" 10,
  {
    identifier = "soul_wall_torch",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 10,
      },
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  register_basic_light "glowstone" 15,
  register_transparent_light "nether_portal" 11,
  {
    identifier = "carved_pumpkin",
    replacement_variants = [facing_nswe],
  } | StandardRegistration,
  {
    identifier = "jack_o_lantern",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      light_info = {
        sky_light_opacity = 'Opaque,
        emission_level = 15,
      },
    },
  } | StandardRegistration,
  register_transparent "cake",
  {
    identifier = "repeater",
    replacement_variants = [
      int_prop "delay" 1 4,
      facing_nswe,
      bool_prop "locked",
      bool_prop "powered",
    ],
    default_override =
      make_kv_list {
        delay = "1",
        facing = "north",
        locked = "false",
        powered = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_stained_glass "white_stained_glass",
  register_stained_glass "orange_stained_glass",
  register_stained_glass "magenta_stained_glass",
  register_stained_glass "light_blue_stained_glass",
  register_stained_glass "yellow_stained_glass",
  register_stained_glass "lime_stained_glass",
  register_stained_glass "pink_stained_glass",
  register_stained_glass "gray_stained_glass",
  register_stained_glass "light_gray_stained_glass",
  register_stained_glass "cyan_stained_glass",
  register_stained_glass "purple_stained_glass",
  register_stained_glass "blue_stained_glass",
  register_stained_glass "brown_stained_glass",
  register_stained_glass "green_stained_glass",
  register_stained_glass "red_stained_glass",
  register_stained_glass "black_stained_glass",
  register_trapdoor "oak_trapdoor",
  register_trapdoor "spruce_trapdoor",
  register_trapdoor "birch_trapdoor",
  register_trapdoor "jungle_trapdoor",
  register_trapdoor "acacia_trapdoor",
  register_trapdoor "cherry_trapdoor",
  register_trapdoor "dark_oak_trapdoor",
  register_trapdoor "mangrove_trapdoor",
  register_trapdoor "bamboo_trapdoor",
  register_basic "stone_bricks",
  register_basic "mossy_stone_bricks",
  register_basic "cracked_stone_bricks",
  register_basic "chiseled_stone_bricks",
  register_basic "packed_mud",
  register_basic "mud_bricks",
  register_basic "infested_stone",
  register_basic "infested_cobblestone",
  register_basic "infested_stone_bricks",
  register_basic "infested_mossy_stone_bricks",
  register_basic "infested_cracked_stone_bricks",
  register_basic "infested_chiseled_stone_bricks",
  register_mushroom_block "brown_mushroom_block",
  register_mushroom_block "red_mushroom_block",
  register_mushroom_block "mushroom_stem",
  {
    identifier = "iron_bars",
    custom_variants = [
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      waterlogged,
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        east = "false",
        north = "false",
        south = "false",
        west = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "chain",
    custom_variants = [waterlogged],
    default_override =
      make_kv_list { axis = "y", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "glass_pane",
    custom_variants = [
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      waterlogged,
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        east = "false",
        north = "false",
        south = "false",
        west = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'GlassPane,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_basic "pumpkin",
  register_basic "melon",
  {
    identifier = "attached_pumpkin_stem",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "attached_melon_stem",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_transparent "pumpkin_stem",
  register_transparent "melon_stem",
  {
    identifier = "vine",
    custom_variants = [
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      bool_prop "up",
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        east = "false",
        north = "false",
        south = "false",
        up = "false",
        west = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "glow_lichen",
    custom_variants = [
      bool_prop "down",
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      bool_prop "up",
      waterlogged,
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        east = "false",
        north = "false",
        south = "false",
        west = "false",
        up = "false",
        down = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 7
      },
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  register_fence_gate "oak_fence_gate",
  register_stairs "brick_stairs",
  register_stairs "stone_brick_stairs",
  register_stairs "mud_brick_stairs",
  {
    identifier = "mycelium",
    replacement_variants = [bool_prop "snowy"],
    default_override = make_kv_list { snowy = "false" },
  } | StandardRegistration,
  register_transparent "lily_pad",
  register_basic "nether_bricks",
  register_fence "nether_brick_fence",
  register_stairs "nether_brick_stairs",
  register_transparent "nether_wart",
  register_transparent_light "enchanting_table" 7,
  {
    identifier = "brewing_stand",
    custom_variants = [
      bool_prop "has_bottle_0",
      bool_prop "has_bottle_1",
      bool_prop "has_bottle_2",
    ],
    default_override =
      make_kv_list {
        has_bottle_0 = "false",
        has_bottle_1 = "false",
        has_bottle_2 = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 1
      },
    },
  } | StandardRegistration,
  register_transparent "cauldron",
  register_transparent "water_cauldron",
  register_transparent_light "lava_cauldron" 15,
  register_transparent "powder_snow_cauldron",
  register_basic_light "end_portal" 15,
  {
    identifier = "end_portal_frame",
    replacement_variants = [bool_prop "eye", facing_nswe],
    default_override =
      make_kv_list { eye = "false", facing = "north" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 1
      },
    },
  } | StandardRegistration,
  register_basic "end_stone",
  register_transparent_light "dragon_egg" 1,
  {
    identifier = "redstone_lamp",
    replacement_variants = [lit],
    default_override = make_kv_list { lit = "false" },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "cocoa",
    replacement_variants = [int_prop "age" 0 2, facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_stairs "sandstone_stairs",
  register_basic "emerald_ore",
  register_basic "deepslate_emerald_ore",
  {
    identifier = "ender_chest",
    custom_variants = [facing_nswe, waterlogged],
    default_override =
      make_kv_list { facing = "north", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 7
      },
    },
  } | StandardRegistration,
  {
    identifier = "tripwire_hook",
    replacement_variants = [bool_prop "attached", facing_nswe, powered],
    default_override =
      make_kv_list { attached = "false", facing = "north", powered = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  },
  {
    identifier = "tripwire",
    custom_variants = [
      bool_prop "attached",
      bool_prop "disarmed",
      bool_prop "east",
      bool_prop "north",
      powered,
      bool_prop "south",
      bool_prop "west",
    ],
    skip_properties = ["disarmed", "powered"],
    default_override =
      make_kv_list {
        attached = "false",
        disarmed = "false",
        east = "false",
        north = "false",
        powered = "false",
        south = "false",
        west = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | FullCustomRegistration,
  register_basic "emerald_block",
  register_stairs "spruce_stairs",
  register_stairs "birch_stairs",
  register_stairs "jungle_stairs",
  {
    identifier = "command_block",
    replacement_variants = [bool_prop "conditional", facing_neswud],
    default_override =
      make_kv_list { conditional = "false", facing = "north" },
  } | StandardRegistration,
  register_transparent_light "beacon" 15,
  register_wall "cobblestone_wall",
  register_wall "mossy_cobblestone_wall",
  register_transparent "flower_pot",
  register_transparent "potted_torchflower",
  register_transparent "potted_oak_sapling",
  register_transparent "potted_spruce_sapling",
  register_transparent "potted_birch_sapling",
  register_transparent "potted_jungle_sapling",
  register_transparent "potted_acacia_sapling",
  register_transparent "potted_cherry_sapling",
  register_transparent "potted_dark_oak_sapling",
  register_transparent "potted_mangrove_propagule",
  register_transparent "potted_fern",
  register_transparent "potted_dandelion",
  register_transparent "potted_poppy",
  register_transparent "potted_blue_orchid",
  register_transparent "potted_allium",
  register_transparent "potted_azure_bluet",
  register_transparent "potted_red_tulip",
  register_transparent "potted_orange_tulip",
  register_transparent "potted_white_tulip",
  register_transparent "potted_pink_tulip",
  register_transparent "potted_oxeye_daisy",
  register_transparent "potted_cornflower",
  register_transparent "potted_lily_of_the_valley",
  register_transparent "potted_wither_rose",
  register_transparent "potted_red_mushroom",
  register_transparent "potted_brown_mushroom",
  register_transparent "potted_dead_bush",
  register_transparent "potted_cactus",
  register_transparent "carrots",
  register_transparent "potatoes",
  register_button "oak_button",
  register_button "spruce_button",
  register_button "birch_button",
  register_button "jungle_button",
  register_button "acacia_button",
  register_button "cherry_button",
  register_button "dark_oak_button",
  register_button "mangrove_button",
  register_button "bamboo_button",
  register_head "skeleton_skull",
  register_wall_head "skeleton_wall_skull",
  register_head "wither_skeleton_skull",
  register_wall_head "wither_skeleton_wall_skull",
  register_head "zombie_head",
  register_wall_head "zombie_wall_head",
  register_head "player_head",
  register_wall_head "player_wall_head",
  register_head "creeper_head",
  register_wall_head "creeper_wall_head",
  register_head "dragon_head",
  register_wall_head "dragon_wall_head",
  register_head "piglin_head",
  register_wall_head "piglin_wall_head",
  {
    identifier = "anvil",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "chipped_anvil",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "damaged_anvil",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_chest "trapped_chest",
  register_weighted_pressure_plate "light_weighted_pressure_plate",
  register_weighted_pressure_plate "heavy_weighted_pressure_plate",
  {
    identifier = "comparator",
    replacement_variants = [facing_nswe, enum_prop "mode" ["compare", "subtract"], powered],
    default_override =
      make_kv_list { facing = "north", mode = "compare", powered = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "daylight_detector",
    custom_variants = [int_prop "power" 0 15],
    replacement_variants = [bool_prop "inverted"],
    default_override =
      make_kv_list { inverted = "false", power = "0" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_basic "redstone_block",
  register_basic "nether_quartz_ore",
  {
    identifier = "hopper",
    custom_variants = [
      bool_prop "enabled",
      enum_prop "facing" ["down", "north", "south", "west", "east"],
    ],
    skip_properties = ["enabled"],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | FullCustomRegistration,
  register_basic "quartz_block",
  register_basic "chiseled_quartz_block",
  {
    identifier = "quartz_pillar",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "quartz_stairs",
    custom_variants = [waterlogged],
    replacement_variants = [
      facing_nswe,
      enum_prop "half" ["top", "bottom"],
      enum_prop
        "shape"
        [
          "straight",
          "inner_left",
          "inner_right",
          "outer_left",
          "outer_right",
        ],
    ],
    default_override =
      make_kv_list {
        facing = "north",
        half = "bottom",
        shape = "straight",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "activator_rail",
    custom_variants = [waterlogged],
    replacement_variants = [
      powered,
      enum_prop
        "shape"
        [
          "north_south",
          "east_west",
          "ascending_east",
          "ascending_west",
          "ascending_north",
          "ascending_south",
        ],
    ],
    default_override =
      make_kv_list {
        powered = "false",
        shape = "north_south",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "dropper",
    custom_variants = [bool_prop "triggered"],
    replacement_variants = [facing_neswud],
    default_override =
      make_kv_list { facing = "north", triggered = "false" },
  } | StandardRegistration,
  register_basic "white_terracotta",
  register_basic "orange_terracotta",
  register_basic "magenta_terracotta",
  register_basic "light_blue_terracotta",
  register_basic "yellow_terracotta",
  register_basic "lime_terracotta",
  register_basic "pink_terracotta",
  register_basic "gray_terracotta",
  register_basic "light_gray_terracotta",
  register_basic "cyan_terracotta",
  register_basic "purple_terracotta",
  register_basic "blue_terracotta",
  register_basic "brown_terracotta",
  register_basic "green_terracotta",
  register_basic "red_terracotta",
  register_basic "black_terracotta",
  register_glass_pane "white_stained_glass_pane",
  register_glass_pane "orange_stained_glass_pane",
  register_glass_pane "magenta_stained_glass_pane",
  register_glass_pane "light_blue_stained_glass_pane",
  register_glass_pane "yellow_stained_glass_pane",
  register_glass_pane "lime_stained_glass_pane",
  register_glass_pane "pink_stained_glass_pane",
  register_glass_pane "gray_stained_glass_pane",
  register_glass_pane "light_gray_stained_glass_pane",
  register_glass_pane "cyan_stained_glass_pane",
  register_glass_pane "purple_stained_glass_pane",
  register_glass_pane "blue_stained_glass_pane",
  register_glass_pane "brown_stained_glass_pane",
  register_glass_pane "green_stained_glass_pane",
  register_glass_pane "red_stained_glass_pane",
  register_glass_pane "black_stained_glass_pane",
  register_stairs "acacia_stairs",
  register_stairs "cherry_stairs",
  register_stairs "dark_oak_stairs",
  register_stairs "mangrove_stairs",
  register_stairs "bamboo_stairs",
  register_stairs "bamboo_mosaic_stairs",
  register_transparent "slime_block",
  {
    identifier = "barrier",
    custom_variants = [waterlogged],
    default_override = make_kv_list { waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "light",
    custom_variants = [waterlogged],
    replacement_variants = [int_prop "level" 0 15],
    default_override =
      make_kv_list { level = "15", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = sky_transparent_info,
        },
        conditions = make_kv_list { level = "0" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 1
          },
        },
        conditions = make_kv_list { level = "1" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 2
          },
        },
        conditions = make_kv_list { level = "2" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 3
          },
        },
        conditions = make_kv_list { level = "3" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 4
          },
        },
        conditions = make_kv_list { level = "4" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 5
          },
        },
        conditions = make_kv_list { level = "5" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 6
          },
        },
        conditions = make_kv_list { level = "6" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 7
          },
        },
        conditions = make_kv_list { level = "7" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 8
          },
        },
        conditions = make_kv_list { level = "8" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 9
          },
        },
        conditions = make_kv_list { level = "9" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 10,
          },
        },
        conditions = make_kv_list { level = "10" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 11,
          },
        },
        conditions = make_kv_list { level = "11" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 12,
          },
        },
        conditions = make_kv_list { level = "12" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 13,
          },
        },
        conditions = make_kv_list { level = "13" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 14,
          },
        },
        conditions = make_kv_list { level = "14" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { level = "15" },
      },
    ],
  } | StandardRegistration,
  register_trapdoor "iron_trapdoor",
  register_basic "prismarine",
  register_basic "prismarine_bricks",
  register_basic "dark_prismarine",
  register_stairs "prismarine_stairs",
  register_stairs "prismarine_brick_stairs",
  register_stairs "dark_prismarine_stairs",
  register_slab "prismarine_slab",
  register_slab "prismarine_brick_slab",
  register_slab "dark_prismarine_slab",
  register_transparent_light "sea_lantern" 15,
  {
    identifier = "hay_block",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  register_transparent "white_carpet",
  register_transparent "orange_carpet",
  register_transparent "magenta_carpet",
  register_transparent "light_blue_carpet",
  register_transparent "yellow_carpet",
  register_transparent "lime_carpet",
  register_transparent "pink_carpet",
  register_transparent "gray_carpet",
  register_transparent "light_gray_carpet",
  register_transparent "cyan_carpet",
  register_transparent "purple_carpet",
  register_transparent "blue_carpet",
  register_transparent "brown_carpet",
  register_transparent "green_carpet",
  register_transparent "red_carpet",
  register_transparent "black_carpet",
  register_basic "terracotta",
  register_basic "coal_block",
  register_basic "packed_ice",
  {
    identifier = "sunflower",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "lilac",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "rose_bush",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "peony",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "tall_grass",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "large_fern",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  register_banner "white_banner",
  register_banner "orange_banner",
  register_banner "magenta_banner",
  register_banner "light_blue_banner",
  register_banner "yellow_banner",
  register_banner "lime_banner",
  register_banner "pink_banner",
  register_banner "gray_banner",
  register_banner "light_gray_banner",
  register_banner "cyan_banner",
  register_banner "purple_banner",
  register_banner "blue_banner",
  register_banner "brown_banner",
  register_banner "green_banner",
  register_banner "red_banner",
  register_banner "black_banner",
  register_wall_banner "white_wall_banner",
  register_wall_banner "orange_wall_banner",
  register_wall_banner "magenta_wall_banner",
  register_wall_banner "light_blue_wall_banner",
  register_wall_banner "yellow_wall_banner",
  register_wall_banner "lime_wall_banner",
  register_wall_banner "pink_wall_banner",
  register_wall_banner "gray_wall_banner",
  register_wall_banner "light_gray_wall_banner",
  register_wall_banner "cyan_wall_banner",
  register_wall_banner "purple_wall_banner",
  register_wall_banner "blue_wall_banner",
  register_wall_banner "brown_wall_banner",
  register_wall_banner "green_wall_banner",
  register_wall_banner "red_wall_banner",
  register_wall_banner "black_wall_banner",
  register_basic "red_sandstone",
  register_basic "chiseled_red_sandstone",
  register_basic "cut_red_sandstone",
  register_stairs "red_sandstone_stairs",
  register_slab "oak_slab",
  register_slab "spruce_slab",
  register_slab "birch_slab",
  register_slab "jungle_slab",
  register_slab "acacia_slab",
  register_slab "cherry_slab",
  register_slab "dark_oak_slab",
  register_slab "mangrove_slab",
  register_slab "bamboo_slab",
  register_slab "bamboo_mosaic_slab",
  register_slab "stone_slab",
  register_slab "smooth_stone_slab",
  register_slab "sandstone_slab",
  register_slab "cut_sandstone_slab",
  register_slab "petrified_oak_slab",
  register_slab "cobblestone_slab",
  register_slab "brick_slab",
  register_slab "stone_brick_slab",
  register_slab "mud_brick_slab",
  register_slab "nether_brick_slab",
  register_slab "quartz_slab",
  register_slab "red_sandstone_slab",
  register_slab "cut_red_sandstone_slab",
  register_slab "purpur_slab",
  register_basic "smooth_stone",
  register_basic "smooth_sandstone",
  register_basic "smooth_quartz",
  register_basic "smooth_red_sandstone",
  register_fence_gate "spruce_fence_gate",
  register_fence_gate "birch_fence_gate",
  register_fence_gate "jungle_fence_gate",
  register_fence_gate "acacia_fence_gate",
  register_fence_gate "cherry_fence_gate",
  register_fence_gate "dark_oak_fence_gate",
  register_fence_gate "mangrove_fence_gate",
  register_fence_gate "bamboo_fence_gate",
  register_fence "spruce_fence",
  register_fence "birch_fence",
  register_fence "jungle_fence",
  register_fence "acacia_fence",
  register_fence "cherry_fence",
  register_fence "dark_oak_fence",
  register_fence "mangrove_fence",
  register_fence "bamboo_fence",
  register_door "spruce_door",
  register_door "birch_door",
  register_door "jungle_door",
  register_door "acacia_door",
  register_door "cherry_door",
  register_door "dark_oak_door",
  register_door "mangrove_door",
  register_door "bamboo_door",
  {
    identifier = "end_rod",
    replacement_variants = [facing_neswud],
    default_override = make_kv_list { facing = "up" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 14,
      },
    },
  } | StandardRegistration,
  {
    identifier = "chorus_plant",
    custom_variants = [
      bool_prop "down",
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      bool_prop "up",
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        down = "false",
        east = "false",
        north = "false",
        south = "false",
        up = "false",
        west = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  register_transparent "chorus_flower",
  register_basic "purpur_block",
  {
    identifier = "purpur_pillar",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  register_stairs "purpur_stairs",
  register_basic "end_stone_bricks",
  register_transparent "torchflower_crop",
  {
    identifier = "pitcher_crop",
    replacement_variants = [int_prop "age" 0 4, enum_prop "half" ["upper", "lower"]],
    default_override =
      make_kv_list { age = "0", half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "pitcher_plant",
    replacement_variants = [enum_prop "half" ["upper", "lower"]],
    default_override = make_kv_list { half = "lower" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  register_transparent "beetroots",
  register_transparent "dirt_path",
  register_basic_light "end_gateway" 15,
  {
    identifier = "repeating_command_block",
    replacement_variants = [bool_prop "conditional", facing_neswud],
    default_override =
      make_kv_list { conditional = "false", facing = "north" },
  } | StandardRegistration,
  {
    identifier = "chain_command_block",
    replacement_variants = [bool_prop "conditional", facing_neswud],
    default_override =
      make_kv_list { conditional = "false", facing = "north" },
  } | StandardRegistration,
  register_basic "frosted_ice",
  register_basic_light "magma_block" 3,
  register_basic "nether_wart_block",
  register_basic "red_nether_bricks",
  {
    identifier = "bone_block",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  register_transparent "structure_void",
  {
    identifier = "observer",
    replacement_variants = [facing_neswud, powered],
    default_override =
      make_kv_list { facing = "south", powered = "false" },
  } | StandardRegistration,
  register_shulker_box "shulker_box",
  register_shulker_box "white_shulker_box",
  register_shulker_box "orange_shulker_box",
  register_shulker_box "magenta_shulker_box",
  register_shulker_box "light_blue_shulker_box",
  register_shulker_box "yellow_shulker_box",
  register_shulker_box "lime_shulker_box",
  register_shulker_box "pink_shulker_box",
  register_shulker_box "gray_shulker_box",
  register_shulker_box "light_gray_shulker_box",
  register_shulker_box "cyan_shulker_box",
  register_shulker_box "purple_shulker_box",
  register_shulker_box "blue_shulker_box",
  register_shulker_box "brown_shulker_box",
  register_shulker_box "green_shulker_box",
  register_shulker_box "red_shulker_box",
  register_shulker_box "black_shulker_box",
  register_glazed_terracotta "white_glazed_terracotta",
  register_glazed_terracotta "orange_glazed_terracotta",
  register_glazed_terracotta "magenta_glazed_terracotta",
  register_glazed_terracotta "light_blue_glazed_terracotta",
  register_glazed_terracotta "yellow_glazed_terracotta",
  register_glazed_terracotta "lime_glazed_terracotta",
  register_glazed_terracotta "pink_glazed_terracotta",
  register_glazed_terracotta "gray_glazed_terracotta",
  register_glazed_terracotta "light_gray_glazed_terracotta",
  register_glazed_terracotta "cyan_glazed_terracotta",
  register_glazed_terracotta "purple_glazed_terracotta",
  register_glazed_terracotta "blue_glazed_terracotta",
  register_glazed_terracotta "brown_glazed_terracotta",
  register_glazed_terracotta "green_glazed_terracotta",
  register_glazed_terracotta "red_glazed_terracotta",
  register_glazed_terracotta "black_glazed_terracotta",
  register_basic "white_concrete",
  register_basic "orange_concrete",
  register_basic "magenta_concrete",
  register_basic "light_blue_concrete",
  register_basic "yellow_concrete",
  register_basic "lime_concrete",
  register_basic "pink_concrete",
  register_basic "gray_concrete",
  register_basic "light_gray_concrete",
  register_basic "cyan_concrete",
  register_basic "purple_concrete",
  register_basic "blue_concrete",
  register_basic "brown_concrete",
  register_basic "green_concrete",
  register_basic "red_concrete",
  register_basic "black_concrete",
  register_basic "white_concrete_powder",
  register_basic "orange_concrete_powder",
  register_basic "magenta_concrete_powder",
  register_basic "light_blue_concrete_powder",
  register_basic "yellow_concrete_powder",
  register_basic "lime_concrete_powder",
  register_basic "pink_concrete_powder",
  register_basic "gray_concrete_powder",
  register_basic "light_gray_concrete_powder",
  register_basic "cyan_concrete_powder",
  register_basic "purple_concrete_powder",
  register_basic "blue_concrete_powder",
  register_basic "brown_concrete_powder",
  register_basic "green_concrete_powder",
  register_basic "red_concrete_powder",
  register_basic "black_concrete_powder",
  {
    identifier = "kelp",
    custom_variants = [int_prop "age" 0 25],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_transparent_no_collider "kelp_plant",
  register_basic "dried_kelp_block",
  register_transparent "turtle_egg",
  register_transparent "sniffer_egg",
  register_basic "dead_tube_coral_block",
  register_basic "dead_brain_coral_block",
  register_basic "dead_bubble_coral_block",
  register_basic "dead_fire_coral_block",
  register_basic "dead_horn_coral_block",
  register_basic "tube_coral_block",
  register_basic "brain_coral_block",
  register_basic "bubble_coral_block",
  register_basic "fire_coral_block",
  register_basic "horn_coral_block",
  register_coral "dead_tube_coral",
  register_coral "dead_brain_coral",
  register_coral "dead_bubble_coral",
  register_coral "dead_fire_coral",
  register_coral "dead_horn_coral",
  register_coral "tube_coral",
  register_coral "brain_coral",
  register_coral "bubble_coral",
  register_coral "fire_coral",
  register_coral "horn_coral",
  register_coral "dead_tube_coral_fan",
  register_coral "dead_brain_coral_fan",
  register_coral "dead_bubble_coral_fan",
  register_coral "dead_fire_coral_fan",
  register_coral "dead_horn_coral_fan",
  register_coral "tube_coral_fan",
  register_coral "brain_coral_fan",
  register_coral "bubble_coral_fan",
  register_coral "fire_coral_fan",
  register_coral "horn_coral_fan",
  register_coral_wall_fan "dead_tube_coral_wall_fan",
  register_coral_wall_fan "dead_brain_coral_wall_fan",
  register_coral_wall_fan "dead_bubble_coral_wall_fan",
  register_coral_wall_fan "dead_fire_coral_wall_fan",
  register_coral_wall_fan "dead_horn_coral_wall_fan",
  register_coral_wall_fan "tube_coral_wall_fan",
  register_coral_wall_fan "brain_coral_wall_fan",
  register_coral_wall_fan "bubble_coral_wall_fan",
  register_coral_wall_fan "fire_coral_wall_fan",
  register_coral_wall_fan "horn_coral_wall_fan",
  {
    identifier = "sea_pickle",
    replacement_variants = [int_prop "pickles" 1 4, bool_prop "waterlogged"],
    default_override =
      make_kv_list { pickles = "1", waterlogged = "true" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 15,
      },
    },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { pickles = "4", waterlogged = "true" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 12,
          },
        },
        conditions = make_kv_list { pickles = "3", waterlogged = "true" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 9
          },
        },
        conditions = make_kv_list { pickles = "2", waterlogged = "true" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 6
          },
        },
        conditions = make_kv_list { pickles = "1", waterlogged = "true" },
      },
    ],
  } | StandardRegistration,
  register_basic "blue_ice",
  {
    identifier = "conduit",
    custom_variants = [waterlogged],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 15,
      },
    },
  } | StandardRegistration,
  register_transparent "bamboo_sapling",
  {
    identifier = "bamboo",
    custom_variants = [
      int_prop "age" 0 1,
      enum_prop "leaves" ["none", "small", "large"],
      int_prop "stage" 0 1,
    ],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_transparent "potted_bamboo",
  {
    identifier = "void_air",
    properties = { air_like = true },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "cave_air",
    properties = { air_like = true },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "bubble_column",
    custom_variants = [bool_prop "drag"],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_stairs "polished_granite_stairs",
  register_stairs "smooth_red_sandstone_stairs",
  register_stairs "mossy_stone_brick_stairs",
  register_stairs "polished_diorite_stairs",
  register_stairs "mossy_cobblestone_stairs",
  register_stairs "end_stone_brick_stairs",
  register_stairs "stone_stairs",
  register_stairs "smooth_sandstone_stairs",
  register_stairs "smooth_quartz_stairs",
  register_stairs "granite_stairs",
  register_stairs "andesite_stairs",
  register_stairs "red_nether_brick_stairs",
  register_stairs "polished_andesite_stairs",
  register_stairs "diorite_stairs",
  register_slab "polished_granite_slab",
  register_slab "smooth_red_sandstone_slab",
  register_slab "mossy_stone_brick_slab",
  register_slab "polished_diorite_slab",
  register_slab "mossy_cobblestone_slab",
  register_slab "end_stone_brick_slab",
  register_slab "smooth_sandstone_slab",
  register_slab "smooth_quartz_slab",
  register_slab "granite_slab",
  register_slab "andesite_slab",
  register_slab "red_nether_brick_slab",
  register_slab "polished_andesite_slab",
  register_slab "diorite_slab",
  register_wall "brick_wall",
  register_wall "prismarine_wall",
  register_wall "red_sandstone_wall",
  register_wall "mossy_stone_brick_wall",
  register_wall "granite_wall",
  register_wall "stone_brick_wall",
  register_wall "mud_brick_wall",
  register_wall "nether_brick_wall",
  register_wall "andesite_wall",
  register_wall "red_nether_brick_wall",
  register_wall "sandstone_wall",
  register_wall "end_stone_brick_wall",
  register_wall "diorite_wall",
  {
    identifier = "scaffolding",
    custom_variants = [int_prop "distance" 0 7, waterlogged],
    replacement_variants = [bool_prop "bottom"],
    default_override =
      make_kv_list { bottom = "false", distance = "7", waterlogged = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "loom",
    replacement_variants = [facing_nswe],
  } | StandardRegistration,
  {
    identifier = "barrel",
    replacement_variants = [facing_neswud, bool_prop "open"],
    default_override =
      make_kv_list { facing = "north", open = "false" },
  } | StandardRegistration,
  {
    identifier = "smoker",
    replacement_variants = [facing_nswe, lit],
    default_override =
      make_kv_list { facing = "north", lit = "false" },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 13,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "blast_furnace",
    replacement_variants = [facing_nswe, lit],
    default_override =
      make_kv_list { facing = "north", lit = "false" },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 13,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  register_basic "cartography_table",
  register_basic "fletching_table",
  {
    identifier = "grindstone",
    replacement_variants = [enum_prop "face" ["floor", "wall", "ceiling"], facing_nswe],
    default_override =
      make_kv_list { face = "wall", facing = "north" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "lectern",
    custom_variants = [bool_prop "has_book", powered],
    replacement_variants = [facing_nswe],
    default_override =
      make_kv_list { facing = "north", has_book = "false", powered = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_basic "smithing_table",
  {
    identifier = "stonecutter",
    replacement_variants = [facing_nswe],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "bell",
    custom_variants = [powered],
    replacement_variants = [
      enum_prop
        "attachment"
        ["floor", "ceiling", "single_wall", "double_wall"],
      facing_nswe,
    ],
    default_override =
      make_kv_list {
        attachment = "floor",
        facing = "north",
        powered = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_lantern "lantern" 15,
  register_lantern "soul_lantern" 10,
  {
    identifier = "campfire",
    custom_variants = [bool_prop "signal_fire", waterlogged],
    replacement_variants = [facing_nswe, lit],
    default_override =
      make_kv_list {
        facing = "north",
        lit = "true",
        signal_fire = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  {
    identifier = "soul_campfire",
    custom_variants = [bool_prop "signal_fire", waterlogged],
    replacement_variants = [facing_nswe, lit],
    default_override =
      make_kv_list {
        facing = "north",
        lit = "true",
        signal_fire = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 10,
          },
        },
        conditions = make_kv_list { lit = "true" },
      },
    ],
  } | StandardRegistration,
  register_transparent_no_collider "sweet_berry_bush",
  {
    identifier = "warped_stem",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "stripped_warped_stem",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "warped_hyphae",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "stripped_warped_hyphae",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  register_basic "warped_nylium",
  register_basic "warped_fungus",
  register_basic "warped_wart_block",
  register_basic "warped_roots",
  register_basic "nether_sprouts",
  {
    identifier = "crimson_stem",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "stripped_crimson_stem",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "crimson_hyphae",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  {
    identifier = "stripped_crimson_hyphae",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  register_basic "crimson_nylium",
  register_basic "crimson_fungus",
  register_basic_light "shroomlight" 15,
  {
    identifier = "weeping_vines",
    custom_variants = [age_0_25],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_transparent "weeping_vines_plant",
  {
    identifier = "twisting_vines",
    custom_variants = [age_0_25],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_transparent "twisting_vines_plant",
  register_basic "crimson_roots",
  register_basic "crimson_planks",
  register_basic "warped_planks",
  register_slab "crimson_slab",
  register_slab "warped_slab",
  register_pressure_plate "crimson_pressure_plate",
  register_pressure_plate "warped_pressure_plate",
  register_fence "crimson_fence",
  register_fence "warped_fence",
  register_trapdoor "crimson_trapdoor",
  register_trapdoor "warped_trapdoor",
  register_fence_gate "crimson_fence_gate",
  register_fence_gate "warped_fence_gate",
  register_stairs "crimson_stairs",
  register_stairs "warped_stairs",
  register_button "crimson_button",
  register_button "warped_button",
  register_door "crimson_door",
  register_door "warped_door",
  register_sign "crimson_sign",
  register_sign "warped_sign",
  register_wall_sign "crimson_wall_sign",
  register_wall_sign "warped_wall_sign",
  {
    identifier = "structure_block",
    replacement_variants = [enum_prop "mode" ["save", "load", "corner", "data"]],
    default_override = make_kv_list { mode = "load" },
  } | StandardRegistration,
  {
    identifier = "jigsaw",
    replacement_variants = [
      enum_prop
        "orientation"
        [
          "down_east",
          "down_north",
          "down_south",
          "down_west",
          "up_east",
          "up_north",
          "up_south",
          "up_west",
          "west_up",
          "east_up",
          "north_up",
          "south_up",
        ],
    ],
    default_override =
      make_kv_list { orientation = "north_up" },
  } | StandardRegistration,
  {
    identifier = "composter",
    custom_variants = [int_prop "level" 0 8],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "target",
    custom_variants = [int_prop "power" 0 15],
  } | StandardRegistration,
  {
    identifier = "bee_nest",
    replacement_variants = [facing_nswe, int_prop "honey_level" 0 5],
  } | StandardRegistration,
  {
    identifier = "beehive",
    replacement_variants = [facing_nswe, int_prop "honey_level" 0 5],
  } | StandardRegistration,
  register_transparent "honey_block",
  register_basic "honeycomb_block",
  register_basic "netherite_block",
  register_basic "ancient_debris",
  register_basic_light "crying_obsidian" 10,
  {
    identifier = "respawn_anchor",
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 15,
          },
        },
        conditions = make_kv_list { charges = "4" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 11,
          },
        },
        conditions = make_kv_list { charges = "3" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 7
          },
        },
        conditions = make_kv_list { charges = "2" },
      },
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Opaque,
            emission_level = 3
          },
        },
        conditions = make_kv_list { charges = "1" },
      },
    ],
  } | StandardRegistration,
  register_transparent "potted_crimson_fungus",
  register_transparent "potted_warped_fungus",
  register_transparent "potted_crimson_roots",
  register_transparent "potted_warped_roots",
  register_basic "lodestone",
  register_basic "blackstone",
  register_stairs "blackstone_stairs",
  register_wall "blackstone_wall",
  register_slab "blackstone_slab",
  register_basic "polished_blackstone",
  register_basic "polished_blackstone_bricks",
  register_basic "cracked_polished_blackstone_bricks",
  register_basic "chiseled_polished_blackstone",
  register_slab "polished_blackstone_brick_slab",
  register_stairs "polished_blackstone_brick_stairs",
  register_wall "polished_blackstone_brick_wall",
  register_basic "gilded_blackstone",
  register_stairs "polished_blackstone_stairs",
  register_slab "polished_blackstone_slab",
  register_pressure_plate "polished_blackstone_pressure_plate",
  register_button "polished_blackstone_button",
  register_wall "polished_blackstone_wall",
  register_basic "chiseled_nether_bricks",
  register_basic "cracked_nether_bricks",
  register_basic "quartz_bricks",
  register_candle "candle",
  register_candle "white_candle",
  register_candle "orange_candle",
  register_candle "magenta_candle",
  register_candle "light_blue_candle",
  register_candle "yellow_candle",
  register_candle "lime_candle",
  register_candle "pink_candle",
  register_candle "gray_candle",
  register_candle "light_gray_candle",
  register_candle "cyan_candle",
  register_candle "purple_candle",
  register_candle "blue_candle",
  register_candle "brown_candle",
  register_candle "green_candle",
  register_candle "red_candle",
  register_candle "black_candle",
  register_candle_cake "candle_cake",
  register_candle_cake "white_candle_cake",
  register_candle_cake "orange_candle_cake",
  register_candle_cake "magenta_candle_cake",
  register_candle_cake "light_blue_candle_cake",
  register_candle_cake "yellow_candle_cake",
  register_candle_cake "lime_candle_cake",
  register_candle_cake "pink_candle_cake",
  register_candle_cake "gray_candle_cake",
  register_candle_cake "light_gray_candle_cake",
  register_candle_cake "cyan_candle_cake",
  register_candle_cake "purple_candle_cake",
  register_candle_cake "blue_candle_cake",
  register_candle_cake "brown_candle_cake",
  register_candle_cake "green_candle_cake",
  register_candle_cake "red_candle_cake",
  register_candle_cake "black_candle_cake",
  register_basic "amethyst_block",
  register_basic "budding_amethyst",
  {
    identifier = "amethyst_cluster",
    custom_variants = [waterlogged],
    replacement_variants = [facing_neswud],
    default_override =
      make_kv_list { facing = "up", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 5
      },
    },
  } | StandardRegistration,
  register_amethyst_bud "large_amethyst_bud" 5,
  register_amethyst_bud "medium_amethyst_bud" 2,
  register_amethyst_bud "small_amethyst_bud" 1,
  register_basic "tuff",
  register_slab "tuff_slab",
  register_stairs "tuff_stairs",
  register_wall "tuff_wall",
  register_basic "polished_tuff",
  register_slab "polished_tuff_slab",
  register_stairs "polished_tuff_stairs",
  register_wall "polished_tuff_wall",
  register_basic "chiseled_tuff",
  register_basic "tuff_bricks",
  register_slab "tuff_brick_slab",
  register_stairs "tuff_brick_stairs",
  register_wall "tuff_brick_wall",
  register_basic "chiseled_tuff_bricks",
  register_basic "calcite",
  register_transparent "tinted_glass",
  register_basic "powder_snow",
  {
    identifier = "sculk_sensor",
    custom_variants = [
      int_prop "power" 0 15,
      enum_prop "sculk_sensor_phase" ["inactive", "active", "cooldown"],
      waterlogged,
    ],
    skip_properties = ["power", "waterlogged"],
    default_override =
      make_kv_list {
        power = "0",
        sculk_sensor_phase = "inactive",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = {
        sky_light_opacity = 'Transparent,
        emission_level = 1
      },
    },
  } | FullCustomRegistration,
  {
    identifier = "calibrated_sculk_sensor",
    custom_variants = [
      facing_nswe,
      int_prop "power" 0 15,
      enum_prop "sculk_sensor_phase" ["inactive", "active", "cooldown"],
      waterlogged,
    ],
    skip_properties = ["power", "waterlogged"],
    default_override =
      make_kv_list {
        facing = "north",
        power = "0",
        sculk_sensor_phase = "inactive",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | FullCustomRegistration,
  register_basic "sculk",
  {
    identifier = "sculk_vein",
    custom_variants = [
      bool_prop "down",
      bool_prop "east",
      bool_prop "north",
      bool_prop "south",
      bool_prop "up",
      waterlogged,
      bool_prop "west",
    ],
    default_override =
      make_kv_list {
        down = "false",
        east = "false",
        north = "false",
        south = "false",
        up = "false",
        west = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
      collision_info = empty_collision_info,
    },
  } | StandardRegistration,
  {
    identifier = "sculk_catalyst",
    replacement_variants = [bool_prop "bloom"],
    default_override = make_kv_list { bloom = "false" },
    default_extra_info = {
      light_info = {
        sky_light_opacity = 'Opaque,
        emission_level = 6
      },
    },
  } | StandardRegistration,
  {
    identifier = "sculk_shrieker",
    custom_variants = [bool_prop "shrieking", waterlogged],
    replacement_variants = [bool_prop "can_summon"],
    default_override =
      make_kv_list {
        can_summon = "false",
        shrieking = "false",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_basic "copper_block",
  register_basic "exposed_copper",
  register_basic "weathered_copper",
  register_basic "oxidized_copper",
  register_basic "copper_ore",
  register_basic "deepslate_copper_ore",
  register_basic "oxidized_cut_copper",
  register_basic "weathered_cut_copper",
  register_basic "exposed_cut_copper",
  register_basic "cut_copper",
  register_basic "oxidized_chiseled_copper",
  register_basic "weathered_chiseled_copper",
  register_basic "exposed_chiseled_copper",
  register_basic "chiseled_copper",
  register_basic "waxed_oxidized_chiseled_copper",
  register_basic "waxed_weathered_chiseled_copper",
  register_basic "waxed_exposed_chiseled_copper",
  register_basic "waxed_chiseled_copper",
  register_stairs "oxidized_cut_copper_stairs",
  register_stairs "weathered_cut_copper_stairs",
  register_stairs "exposed_cut_copper_stairs",
  register_stairs "cut_copper_stairs",
  register_slab "oxidized_cut_copper_slab",
  register_slab "weathered_cut_copper_slab",
  register_slab "exposed_cut_copper_slab",
  register_slab "cut_copper_slab",
  register_basic "waxed_copper_block",
  register_basic "waxed_weathered_copper",
  register_basic "waxed_exposed_copper",
  register_basic "waxed_oxidized_copper",
  register_basic "waxed_oxidized_cut_copper",
  register_basic "waxed_weathered_cut_copper",
  register_basic "waxed_exposed_cut_copper",
  register_basic "waxed_cut_copper",
  register_stairs "waxed_oxidized_cut_copper_stairs",
  register_stairs "waxed_weathered_cut_copper_stairs",
  register_stairs "waxed_exposed_cut_copper_stairs",
  register_stairs "waxed_cut_copper_stairs",
  register_slab "waxed_oxidized_cut_copper_slab",
  register_slab "waxed_weathered_cut_copper_slab",
  register_slab "waxed_exposed_cut_copper_slab",
  register_slab "waxed_cut_copper_slab",
  register_door "copper_door",
  register_door "exposed_copper_door",
  register_door "oxidized_copper_door",
  register_door "weathered_copper_door",
  register_door "waxed_copper_door",
  register_door "waxed_exposed_copper_door",
  register_door "waxed_oxidized_copper_door",
  register_door "waxed_weathered_copper_door",
  register_trapdoor "copper_trapdoor",
  register_trapdoor "exposed_copper_trapdoor",
  register_trapdoor "oxidized_copper_trapdoor",
  register_trapdoor "weathered_copper_trapdoor",
  register_trapdoor "waxed_copper_trapdoor",
  register_trapdoor "waxed_exposed_copper_trapdoor",
  register_trapdoor "waxed_oxidized_copper_trapdoor",
  register_trapdoor "waxed_weathered_copper_trapdoor",
  register_grate "copper_grate",
  register_grate "exposed_copper_grate",
  register_grate "weathered_copper_grate",
  register_grate "oxidized_copper_grate",
  register_grate "waxed_copper_grate",
  register_grate "waxed_exposed_copper_grate",
  register_grate "waxed_weathered_copper_grate",
  register_grate "waxed_oxidized_copper_grate",
  register_bulb "copper_bulb" 15,
  register_bulb "exposed_copper_bulb" 12,
  register_bulb "weathered_copper_bulb" 8,
  register_bulb "oxidized_copper_bulb" 4,
  register_bulb "waxed_copper_bulb" 15,
  register_bulb "waxed_exposed_copper_bulb" 12,
  register_bulb "waxed_weathered_copper_bulb" 8,
  register_bulb "waxed_oxidized_copper_bulb" 4,
  {
    identifier = "lightning_rod",
    custom_variants = [waterlogged],
    replacement_variants = [facing_neswud, powered],
    default_override =
      make_kv_list { facing = "up", powered = "false", waterlogged = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "pointed_dripstone",
    custom_variants = [waterlogged],
    replacement_variants = [
      enum_prop
        "thickness"
        ["tip_merge", "tip", "frustum", "middle", "base"],
      enum_prop "vertical_direction" ["up", "down"],
    ],
    default_override =
      make_kv_list {
        thickness = "tip",
        vertical_direction = "up",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_basic "dripstone_block",
  {
    identifier = "cave_vines",
    custom_variants = [age_0_25, bool_prop "berries"],
    skip_properties = ["age"],
    default_override =
      make_kv_list { age = "0", berries = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
    extra_info_modifiers = [
      {
        modifier = {
          light_info = {
            sky_light_opacity = 'Transparent,
            emission_level = 14,
          },
        },
        conditions = make_kv_list { berries = "true" },
      },
    ],
  } | FullCustomRegistration,
  {
    identifier = "cave_vines_plant",
    replacement_variants = [bool_prop "berries"],
    default_override = make_kv_list { berries = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_transparent "spore_blossom",
  register_transparent "azalea",
  register_transparent "flowering_azalea",
  register_transparent "moss_carpet",
  {
    identifier = "pink_petals",
    custom_variants = [facing_nswe, int_prop "flower_amount" 1 4],
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  register_transparent "moss_block",
  {
    identifier = "big_dripleaf",
    custom_variants = [waterlogged],
    replacement_variants = [
      facing_nswe,
      enum_prop "tilt" ["none", "unstable", "partial", "full"],
    ],
    default_override =
      make_kv_list { facing = "north", tilt = "none", waterlogged = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "big_dripleaf_stem",
    custom_variants = [waterlogged],
    replacement_variants = [facing_nswe],
    default_override =
      make_kv_list { facing = "north", waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "small_dripleaf",
    custom_variants = [waterlogged],
    replacement_variants = [facing_nswe, enum_prop "half" ["upper", "lower"]],
    default_override =
      make_kv_list { facing = "north", half = "lower", waterlogged = "false" },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "hanging_roots",
    custom_variants = [waterlogged],
    default_override = make_kv_list { waterlogged = "false" },
  } | StandardRegistration,
  register_basic "rooted_dirt",
  register_basic "mud",
  {
    identifier = "deepslate",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  register_basic "cobbled_deepslate",
  register_stairs "cobbled_deepslate_stairs",
  register_slab "cobbled_deepslate_slab",
  register_wall "cobbled_deepslate_wall",
  register_basic "polished_deepslate",
  register_stairs "polished_deepslate_stairs",
  register_slab "polished_deepslate_slab",
  register_wall "polished_deepslate_wall",
  register_basic "deepslate_tiles",
  register_stairs "deepslate_tile_stairs",
  register_slab "deepslate_tile_slab",
  register_wall "deepslate_tile_wall",
  register_basic "deepslate_bricks",
  register_stairs "deepslate_brick_stairs",
  register_slab "deepslate_brick_slab",
  register_wall "deepslate_brick_wall",
  register_basic "chiseled_deepslate",
  register_basic "cracked_deepslate_bricks",
  register_basic "cracked_deepslate_tiles",
  {
    identifier = "infested_deepslate",
    default_override = make_kv_list { axis = "y" },
  } | StandardRegistration,
  register_basic "smooth_basalt",
  register_basic "raw_iron_block",
  register_basic "raw_copper_block",
  register_basic "raw_gold_block",
  register_transparent "potted_azalea_bush",
  register_transparent "potted_flowering_azalea_bush",
  {
    identifier = "ochre_froglight",
    default_override = make_kv_list { axis = "y" },
    default_extra_info = {
      light_info = {
        sky_light_opacity = 'Opaque,
        emission_level = 15,
      },
    },
  } | StandardRegistration,
  {
    identifier = "verdant_froglight",
    default_override = make_kv_list { axis = "y" },
    default_extra_info = {
      light_info = {
        sky_light_opacity = 'Opaque,
        emission_level = 15,
      },
    },
  } | StandardRegistration,
  {
    identifier = "pearlescent_froglight",
    default_override = make_kv_list { axis = "y" },
    default_extra_info = {
      light_info = {
        sky_light_opacity = 'Opaque,
        emission_level = 15,
      },
    },
  } | StandardRegistration,
  register_transparent "frogspawn",
  register_basic "reinforced_deepslate",
  {
    identifier = "decorated_pot",
    custom_variants = [bool_prop "cracked", facing_nswe, waterlogged],
    default_override =
      make_kv_list {
        cracked = "false",
        facing = "north",
        waterlogged = "false",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "crafter",
    replacement_variants = [
      bool_prop "crafting",
      enum_prop
        "orientation"
        [
          "down_east",
          "down_north",
          "down_south",
          "down_west",
          "up_east",
          "up_north",
          "up_south",
          "up_west",
          "west_up",
          "east_up",
          "north_up",
          "south_up",
        ],
      bool_prop "triggered",
    ],
    default_override =
      make_kv_list {
        crafting = "false",
        orientation = "north_up",
        triggered = "false",
      },
  } | StandardRegistration,
  {
    identifier = "trial_spawner",
    replacement_variants = [
      bool_prop "ominous",
      enum_prop
        "trial_spawner_state"
        [
          "inactive",
          "waiting_for_players",
          "active",
          "waiting_for_reward_ejection",
          "ejecting_reward",
          "cooldown",
        ],
    ],
    default_override =
      make_kv_list { ominous = "false", trial_spawner_state = "inactive" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "vault",
    replacement_variants = [
      facing_nswe,
      bool_prop "ominous",
      enum_prop
        "vault_state"
        ["inactive", "active", "unlocking", "ejecting"],
    ],
    default_override =
      make_kv_list {
        facing = "north",
        ominous = "false",
        vault_state = "inactive",
      },

    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  } | StandardRegistration,
  {
    identifier = "heavy_core",
    custom_variants = [waterlogged],
    default_override = make_kv_list { waterlogged = "false" },
    default_extra_info = {
      opacity = 'Transparent,
      light_info = sky_transparent_info,
    },
  },
]
in

items
