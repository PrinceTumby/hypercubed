-- vim: ts=2:sw=2

-- Common properties
let CustomPropertyType =
      < Boolean : { Boolean : {} }
      | Int : { Int : { start : Natural, end : Natural  }}
      | Enum : { Enum : List Text }
      >

let CustomProperty = { name : Text, prop_type : CustomPropertyType }

let BoolProp = CustomPropertyType.Boolean

let IntProp = CustomPropertyType.Int

let EnumProp = CustomPropertyType.Enum

let boolProp = λ(name : Text) → { name, prop_type = BoolProp { Boolean = {=} } } : CustomProperty

let intProp =
      λ(name : Text) →
      λ(start : Natural) →
      λ(end : Natural) →
        { name, prop_type = IntProp { Int = { start, end } } } : CustomProperty

let enumProp =
      λ(name : Text) →
      λ(variants : List Text) →
        { name, prop_type = EnumProp { Enum = variants } } : CustomProperty

let facing_nswe = enumProp "facing" [ "north", "south", "west", "east" ]

let facing_neswud =
      enumProp "facing" [ "north", "east", "south", "west", "up", "down" ]

let waterlogged = boolProp "waterlogged"

let powered = boolProp "powered"

let lit = boolProp "lit"

let stage_0_1 = intProp "stage" 0 1

let age_0_15 = intProp "age" 0 15

let age_0_25 = intProp "age" 0 25

let rotation_0_15 = intProp "rotation" 0 15

let chest_type = enumProp "type" [ "single", "left", "right" ]

let Properties = { Type = { air_like : Bool }, default.air_like = False }

-- Copied from prelude
let mapList
    : ∀(a : Type) → ∀(b : Type) → (a → b) → List a → List b
    = λ(a : Type) →
      λ(b : Type) →
      λ(f : a → b) →
      λ(xs : List a) →
        List/build
          b
          ( λ(list : Type) →
            λ(cons : b → list → list) →
              List/fold a xs list (λ(x : a) → cons (f x))
          )

let toNewMap =
      mapList
        { mapKey : Text, mapValue : Text }
        { key : Text, value : Text }
        ( λ(entry : { mapKey : Text, mapValue : Text }) →
            { key = entry.mapKey, value = entry.mapValue }
        )

-- Basic types
let BlockOpacity = < Opaque | Leaves | Glass | GlassPane | Transparent >

let SkyLightOpacity = < Opaque | Translucent | Transparent >

let AABB = { corner_1 : List Double, corner_2 : List Double }

let makeAABB =
  λ(start : List Double) →
  λ(end : List Double) →
    { corner_1 = start, corner_2 = end }

let CollisionInfo =
      < Empty : { Empty : {} }
      | FullBlock : { FullBlock : {} }
      | Complex : { Complex : List AABB }
      >

let emptyCollisionInfo = CollisionInfo.Empty { Empty = {=} }

let complexCollisionInfo =
  λ(aabbs : List AABB) →
    CollisionInfo.Complex { Complex = aabbs }

let BlockstateInfo =
      { Type =
          { opacity : BlockOpacity
          , light_info :
              { sky_light_opacity : SkyLightOpacity, emission_level : Natural }
          , collision_info : CollisionInfo
          }
      , default =
        { opacity = BlockOpacity.Opaque
        , light_info =
          { sky_light_opacity = SkyLightOpacity.Opaque, emission_level = 0 }
        , collision_info = CollisionInfo.FullBlock { FullBlock = {=} }
        }
      }

let skyTransparentInfo =
      { sky_light_opacity = SkyLightOpacity.Transparent
      , emission_level = 0
      }

let BlockstateInfoModifier =
      { Type =
          { opacity : Optional BlockOpacity
          , light_info :
              Optional {
                , sky_light_opacity : SkyLightOpacity
                , emission_level : Natural
                }
          , collision_info : Optional CollisionInfo
          }
      , default =
        { opacity = None BlockOpacity
        , light_info =
            None {
              , sky_light_opacity : SkyLightOpacity
              , emission_level : Natural
              }
        , collision_info = None CollisionInfo
        }
      }

let BlockstateInfoModifierCase =
      { modifier : BlockstateInfoModifier.Type
      , conditions : List { key : Text, value : Text }
      }

let StandardRegistration =
      { Type =
          { type : Text
          , identifier : Text
          , custom_variants : Optional (List CustomProperty)
          , replacement_variants : Optional (List CustomProperty)
          , default_override : Optional (List { key : Text, value : Text })
          , properties : Properties.Type
          , default_extra_info : BlockstateInfo.Type
          , extra_info_modifiers : List BlockstateInfoModifierCase
          }
      , default =
        { type = "Standard"
        , custom_variants = None (List CustomProperty)
        , replacement_variants = None (List CustomProperty)
        , default_override = None (List { key : Text, value : Text })
        , properties = Properties.default
        , default_extra_info = BlockstateInfo.default
        , extra_info_modifiers = [] : List BlockstateInfoModifierCase
        }
      }

let FullCustomRegistration =
      { Type =
          { type : Text
          , identifier : Text
          , custom_variants : List CustomProperty
          , skip_properties : List Text
          , default_override : Optional (List { key : Text, value : Text })
          , properties : Properties.Type
          , default_extra_info : BlockstateInfo.Type
          , extra_info_modifiers : List BlockstateInfoModifierCase
          }
      , default =
        { type = "FullCustom"
        , default_override = None (List { key : Text, value : Text })
        , properties = Properties.default
        , default_extra_info = BlockstateInfo.default
        , extra_info_modifiers = [] : List BlockstateInfoModifierCase
        }
      }

let LiquidRegistration =
      { Type =
          { type : Text
          , identifier : Text
          , properties : Properties.Type
          , default_extra_info : BlockstateInfo.Type
          , extra_info_modifiers : List BlockstateInfoModifierCase
          }
      , default =
        { type = "Liquid"
        , properties = Properties.default
        , default_extra_info = BlockstateInfo.default
        , extra_info_modifiers = [] : List BlockstateInfoModifierCase
        }
      }

let Registration =
      < Standard : StandardRegistration.Type
      | FullCustom : FullCustomRegistration.Type
      | Liquid : LiquidRegistration.Type
      >

let registerBasic =
      λ(identifier : Text) →
        Registration.Standard StandardRegistration::{ identifier }

let registerBasicLiquid =
      λ(identifier : Text) →
        Registration.Liquid
          LiquidRegistration::{
          , identifier
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = {
              , sky_light_opacity = SkyLightOpacity.Translucent
              , emission_level = 0
              }
            }
          }

let registerBasicLiquidLight =
      λ(identifier : Text) →
      λ(emission_level : Natural) →
        Registration.Liquid
          LiquidRegistration::{
          , identifier
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = {
              , sky_light_opacity = SkyLightOpacity.Translucent
              , emission_level
              }
            }
          }

let registerBasicLight =
      λ(identifier : Text) →
      λ(emission_level : Natural) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , default_extra_info = BlockstateInfo::{
            , light_info = {
              , sky_light_opacity = SkyLightOpacity.Opaque
              , emission_level
              }
            }
          }

let registerTransparent =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            }
          }

let registerTransparentLight =
      λ(identifier : Text) →
      λ(emission_level : Natural) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = {
              , sky_light_opacity = SkyLightOpacity.Transparent
              , emission_level
              }
            }
          }

let registerTransparentNoCollider =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            , collision_info = emptyCollisionInfo
            }
          }

let registerTransparentLightNoCollider =
      λ(identifier : Text) →
      λ(emission_level : Natural) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = {
              , sky_light_opacity = SkyLightOpacity.Transparent
              , emission_level
              }
            , collision_info = emptyCollisionInfo
            }
          }

let registerLog =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , default_override = Some (toNewMap (toMap { axis = "y" }))
          }

let registerLeaves =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some
            [ intProp "distance" 1 7, boolProp "persistent", waterlogged ]
          , default_override = Some
              ( toNewMap
                  ( toMap
                      { distance = "7"
                      , persistent = "false"
                      , waterlogged = "false"
                      }
                  )
              )
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Leaves
            , light_info = {
              , sky_light_opacity = SkyLightOpacity.Translucent
              , emission_level = 0
              }
            }
          }

let registerBed =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , default_override = Some
              ( toNewMap
                  ( toMap
                      { facing = "north", occupied = "false", part = "foot" }
                  )
              )
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , collision_info = complexCollisionInfo [
              , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.5625, 1.0 ]
              ]
            }
          }

let registerSlab =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ waterlogged ]
          , replacement_variants = Some
            [ enumProp "type" [ "top", "bottom", "double" ] ]
          , default_override = Some
              (toNewMap (toMap { type = "bottom", waterlogged = "false" }))
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            }
          , extra_info_modifiers =
            [ { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.5, 1.0 ]
                  ])
                }
              , conditions = toNewMap (toMap { type = "bottom" })
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.5, 0.0 ] [ 1.0, 1.0, 1.0 ]
                  ])
                }
              , conditions = toNewMap (toMap { type = "top" })
              }
            ]
          }

let registerStairs =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ waterlogged ]
          , replacement_variants = Some
            [ facing_nswe
            , enumProp "half" [ "top", "bottom" ]
            , enumProp
                "shape"
                [ "straight"
                , "inner_left"
                , "inner_right"
                , "outer_left"
                , "outer_right"
                ]
            ]
          , default_override = Some
              ( toNewMap
                  ( toMap
                      { facing = "north"
                      , half = "bottom"
                      , shape = "straight"
                      , waterlogged = "false"
                      }
                  )
              )
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            }
          , extra_info_modifiers =
            [ { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.5, 1.0 ]
                  , makeAABB [ 0.5, 0.5, 0.0 ] [ 1.0, 1.0, 1.0 ]
                  ])
                }
              , conditions = toNewMap (toMap { shape = "straight" })
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.5, 1.0 ]
                  , makeAABB [ 0.5, 0.5, 0.0 ] [ 1.0, 1.0, 1.0 ]
                  , makeAABB [ 0.0, 0.5, 0.0 ] [ 0.5, 1.0, 0.5 ]
                  ])
                }
              , conditions = toNewMap (toMap { shape = "inner_left" })
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.5, 1.0 ]
                  , makeAABB [ 0.5, 0.5, 0.0 ] [ 1.0, 1.0, 1.0 ]
                  , makeAABB [ 0.0, 0.5, 0.5 ] [ 0.5, 1.0, 1.0 ]
                  ])
                }
              , conditions = toNewMap (toMap { shape = "inner_right" })
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.5, 1.0 ]
                  , makeAABB [ 0.5, 0.5, 0.5 ] [ 1.0, 1.0, 1.0 ]
                  ])
                }
              , conditions = toNewMap (toMap { shape = "outer_left" })
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.5, 1.0 ]
                  , makeAABB [ 0.5, 0.5, 0.5 ] [ 1.0, 1.0, 1.0 ]
                  ])
                }
              , conditions = toNewMap (toMap { shape = "outer_right" })
              }
            ]
          }

let registerFence =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some
            [ boolProp "east"
            , boolProp "north"
            , boolProp "south"
            , waterlogged
            , boolProp "west"
            ]
          , default_override = Some
              ( toNewMap
                  ( toMap
                      { east = "false"
                      , north = "false"
                      , south = "false"
                      , waterlogged = "false"
                      , west = "false"
                      }
                  )
              )
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            }
          , extra_info_modifiers =
            [ { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.375, 0.0, 0.375 ] [ 0.625, 1.5, 0.625 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "false"
                  , north = "false"
                  , south = "false"
                  , west = "false"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.375, 0.0, 0.375 ] [ 1.0, 1.5, 0.625 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "true"
                  , north = "false"
                  , south = "false"
                  , west = "false"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.375, 0.0, 0.0 ] [ 0.625, 1.5, 0.625 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "false"
                  , north = "true"
                  , south = "false"
                  , west = "false"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.375, 0.0, 0.375 ] [ 1.0, 1.5, 0.625 ]
                  , makeAABB [ 0.375, 0.0, 0.0 ] [ 0.625, 1.5, 0.375 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "true"
                  , north = "true"
                  , south = "false"
                  , west = "false"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.375, 0.0, 0.375 ] [ 0.625, 1.5, 1.0 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "false"
                  , north = "false"
                  , south = "true"
                  , west = "false"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.375, 0.0, 0.375 ] [ 1.0, 1.5, 1.0 ]
                  , makeAABB [ 0.375, 0.0, 0.625 ] [ 1.0, 1.5, 1.0 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "true"
                  , north = "false"
                  , south = "true"
                  , west = "false"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.375, 0.0, 0.0 ] [ 0.625, 1.5, 1.0 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "false"
                  , north = "true"
                  , south = "true"
                  , west = "false"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.375, 0.0, 0.375 ] [ 1.0, 1.5, 0.625 ]
                  , makeAABB [ 0.375, 0.0, 0.0 ] [ 0.625, 1.5, 1.0 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "true"
                  , north = "true"
                  , south = "true"
                  , west = "false"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.375 ] [ 0.625, 1.5, 0.625 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "false"
                  , north = "false"
                  , south = "false"
                  , west = "true"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.375 ] [ 1.0, 1.5, 0.625 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "true"
                  , north = "false"
                  , south = "false"
                  , west = "true"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.375 ] [ 0.375, 1.5, 0.625 ]
                  , makeAABB [ 0.375, 0.0, 0.0 ] [ 0.625, 1.5, 0.625 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "false"
                  , north = "true"
                  , south = "false"
                  , west = "true"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.375 ] [ 1.0, 1.5, 0.625 ]
                  , makeAABB [ 0.375, 0.0, 0.0 ] [ 0.625, 1.5, 0.375 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "true"
                  , north = "true"
                  , south = "false"
                  , west = "true"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.375 ] [ 0.375, 1.5, 0.625 ]
                  , makeAABB [ 0.375, 0.0, 0.375 ] [ 0.625, 1.5, 1.0 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "false"
                  , north = "false"
                  , south = "true"
                  , west = "true"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.375 ] [ 1.0, 1.5, 0.625 ]
                  , makeAABB [ 0.375, 0.0, 0.625 ] [ 1.0, 1.5, 1.0 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "true"
                  , north = "false"
                  , south = "true"
                  , west = "true"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.375 ] [ 0.625, 1.5, 0.625 ]
                  , makeAABB [ 0.375, 0.0, 0.0 ] [ 0.625, 1.5, 1.0 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "false"
                  , north = "true"
                  , south = "true"
                  , west = "true"
                  }
                )
              }
            , { modifier = BlockstateInfoModifier::{
                , collision_info = Some (complexCollisionInfo [
                  , makeAABB [ 0.0, 0.0, 0.375 ] [ 1.0, 1.5, 0.625 ]
                  , makeAABB [ 0.375, 0.0, 0.0 ] [ 0.625, 1.5, 1.0 ]
                  ])
                }
              , conditions = toNewMap
                ( toMap
                  { east = "true"
                  , north = "true"
                  , south = "true"
                  , west = "true"
                  }
                )
              }
            ]
          }

let registerFenceGate =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ powered ]
          , replacement_variants = Some
            [ facing_nswe, boolProp "in_wall", boolProp "open" ]
          , default_override = Some
              ( toNewMap
                  ( toMap
                      { facing = "north"
                      , in_wall = "false"
                      , open = "false"
                      , powered = "false"
                      }
                  )
              )
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            }
          }

let registerWall =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some
            [ enumProp "east" [ "none", "low", "tall" ]
            , enumProp "north" [ "none", "low", "tall" ]
            , enumProp "south" [ "none", "low", "tall" ]
            , boolProp "up"
            , waterlogged
            , enumProp "west" [ "none", "low", "tall" ]
            ]
          , default_override = Some
              ( toNewMap
                  ( toMap
                      { east = "none"
                      , north = "none"
                      , south = "none"
                      , up = "true"
                      , waterlogged = "false"
                      , west = "none"
                      }
                  )
              )
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            }
          }

let registerDoor =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ powered ]
          , replacement_variants = Some
            [ facing_nswe
            , enumProp "half" [ "upper", "lower" ]
            , enumProp "hinge" [ "left", "right" ]
            , boolProp "open"
            ]
          , default_override = Some
              ( toNewMap
                  ( toMap
                      { facing = "north"
                      , half = "lower"
                      , hinge = "left"
                      , open = "false"
                      , powered = "false"
                      }
                  )
              )
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            }
          }

let registerTrapdoor =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ powered, waterlogged ]
          , replacement_variants = Some
            [ facing_nswe
            , enumProp "half" [ "top", "bottom" ]
            , boolProp "open"
            ]
          , default_override = Some
              ( toNewMap
                  ( toMap
                      { facing = "north"
                      , half = "bottom"
                      , open = "false"
                      , powered = "false"
                      , waterlogged = "false"
                      }
                  )
              )
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            }
          }

let registerChest =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ facing_nswe, chest_type, waterlogged ]
          , default_override = Some
              ( toNewMap
                  ( toMap
                      { facing = "north"
                      , type = "single"
                      , waterlogged = "false"
                      }
                  )
              )
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            }
          }

let registerSign =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ rotation_0_15, waterlogged ]
          , default_override = Some
              (toNewMap (toMap { rotation = "0", waterlogged = "false" }))
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            , collision_info = emptyCollisionInfo
            }
          }

let registerWallSign =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ facing_nswe, waterlogged ]
          , default_override = Some
              (toNewMap (toMap { facing = "north", waterlogged = "false" }))
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            , collision_info = emptyCollisionInfo
            }
          }

let registerHangingSign =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some
            [ boolProp "attached", rotation_0_15, waterlogged ]
          , default_override = Some
              ( toNewMap
                  ( toMap
                      { attached = "false"
                      , rotation = "0"
                      , waterlogged = "false"
                      }
                  )
              )
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            }
          }

let registerWallHangingSign =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ facing_nswe, waterlogged ]
          , default_override = Some
              (toNewMap (toMap { facing = "north", waterlogged = "false" }))
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            }
          }

let registerStainedGlass =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            }
          }

let registerGrate =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ waterlogged ]
          , default_override = Some (toNewMap (toMap { waterlogged = "false" }))
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            }
          }

let registerPressurePlate =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , replacement_variants = Some [ powered ]
          , default_override = Some (toNewMap (toMap { powered = "false" }))
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            , collision_info = emptyCollisionInfo
            }
          }

let registerButton =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , replacement_variants = Some
            [ enumProp "face" [ "floor", "wall", "ceiling" ]
            , facing_nswe
            , powered
            ]
          , default_override = Some
              ( toNewMap
                  (toMap { face = "wall", facing = "north", powered = "false" })
              )
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            , collision_info = emptyCollisionInfo
            }
          }

let registerMushroomBlock =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some
            [ boolProp "down"
            , boolProp "east"
            , boolProp "north"
            , boolProp "south"
            , boolProp "up"
            , boolProp "west"
            ]
          }

let registerHead =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ powered, rotation_0_15 ]
          , default_override = Some
              (toNewMap (toMap { powered = "false", rotation = "0" }))
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            }
          }

let registerWallHead =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ facing_nswe, powered ]
          , default_override = Some
              (toNewMap (toMap { facing = "north", powered = "false" }))
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            }
          }

let registerWeightedPressurePlate =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , replacement_variants = Some [ intProp "power" 0 15 ]
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            , collision_info = emptyCollisionInfo
            }
          }

let registerGlassPane =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some
            [ boolProp "east"
            , boolProp "north"
            , boolProp "south"
            , waterlogged
            , boolProp "west"
            ]
          , default_override = Some
              ( toNewMap
                  ( toMap
                      { east = "false"
                      , north = "false"
                      , south = "false"
                      , waterlogged = "false"
                      , west = "false"
                      }
                  )
              )
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            }
          }

let registerBanner =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ rotation_0_15 ]
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            , collision_info = emptyCollisionInfo
            }
          }

let registerWallBanner =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ facing_nswe ]
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            , collision_info = emptyCollisionInfo
            }
          }

let registerShulkerBox =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ facing_neswud ]
          , default_override = Some (toNewMap (toMap { facing = "up" }))
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            }
          }

let registerGlazedTerracotta =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , replacement_variants = Some [ facing_nswe ]
          }

let registerCoral =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ waterlogged ]
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            , collision_info = emptyCollisionInfo
            }
          }

let registerCoralWallFan =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , replacement_variants = Some [ facing_nswe ]
          , custom_variants = Some [ waterlogged ]
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            }
          }

let registerLantern =
      λ(identifier : Text) →
      λ(emission_level : Natural) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , replacement_variants = Some [ boolProp "hanging" ]
          , custom_variants = Some [ waterlogged ]
          , default_override = Some
              (toNewMap (toMap { hanging = "false", waterlogged = "false" }))
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = {
              , sky_light_opacity = SkyLightOpacity.Transparent
              , emission_level
              }
            }
          }

let registerCandle =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , replacement_variants = Some [ intProp "candles" 1 4, lit ]
          , custom_variants = Some [ waterlogged ]
          , default_override = Some
              ( toNewMap
                  ( toMap
                      { candles = "1", lit = "false", waterlogged = "false" }
                  )
              )
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            }
          , extra_info_modifiers =
            [ { modifier = BlockstateInfoModifier::{
                , light_info = Some {
                  , sky_light_opacity = SkyLightOpacity.Transparent
                  , emission_level = 12
                  }
                }
              , conditions = toNewMap (toMap { candles = "4", lit = "true" })
              }
            , { modifier = BlockstateInfoModifier::{
                , light_info = Some {
                  , sky_light_opacity = SkyLightOpacity.Transparent
                  , emission_level = 9
                  }
                }
              , conditions = toNewMap (toMap { candles = "3", lit = "true" })
              }
            , { modifier = BlockstateInfoModifier::{
                , light_info = Some {
                  , sky_light_opacity = SkyLightOpacity.Transparent
                  , emission_level = 6
                  }
                }
              , conditions = toNewMap (toMap { candles = "2", lit = "true" })
              }
            , { modifier = BlockstateInfoModifier::{
                , light_info = Some {
                  , sky_light_opacity = SkyLightOpacity.Transparent
                  , emission_level = 3
                  }
                }
              , conditions = toNewMap (toMap { candles = "1", lit = "true" })
              }
            ]
          }

let registerCandleCake =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , replacement_variants = Some [ lit ]
          , default_override = Some (toNewMap (toMap { lit = "false" }))
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = skyTransparentInfo
            }
          }

let registerAmethystBud =
      λ(identifier : Text) →
      λ(emission_level : Natural) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , custom_variants = Some [ waterlogged ]
          , replacement_variants = Some [ facing_neswud ]
          , default_override = Some
              (toNewMap (toMap { facing = "up", waterlogged = "false" }))
          , default_extra_info = BlockstateInfo::{
            , opacity = BlockOpacity.Transparent
            , light_info = {
              , sky_light_opacity = SkyLightOpacity.Transparent
              , emission_level
              }
            }
          }

let registerBulb =
      λ(identifier : Text) →
        Registration.Standard
          StandardRegistration::{
          , identifier
          , replacement_variants = Some [ lit, powered ]
          , default_override = Some
              (toNewMap (toMap { lit = "false", powered = "false" }))
          }

in  [ Registration.Standard
        StandardRegistration::{
        , identifier = "air"
        , properties = Properties::{ air_like = True }
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 0
            }
          , collision_info = emptyCollisionInfo
          }
        }
    , registerBasic "stone"
    , registerBasic "granite"
    , registerBasic "polished_granite"
    , registerBasic "diorite"
    , registerBasic "polished_diorite"
    , registerBasic "andesite"
    , registerBasic "polished_andesite"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "grass_block"
        , default_override = Some (toNewMap (toMap { snowy = "false" }))
        , replacement_variants = Some [ boolProp "snowy" ]
        }
    , registerBasic "dirt"
    , registerBasic "coarse_dirt"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "podzol"
        , default_override = Some (toNewMap (toMap { snowy = "false" }))
        , replacement_variants = Some [ boolProp "snowy" ]
        }
    , registerBasic "cobblestone"
    , registerBasic "oak_planks"
    , registerBasic "spruce_planks"
    , registerBasic "birch_planks"
    , registerBasic "jungle_planks"
    , registerBasic "acacia_planks"
    , registerBasic "cherry_planks"
    , registerBasic "dark_oak_planks"
    , registerBasic "mangrove_planks"
    , registerBasic "bamboo_planks"
    , registerBasic "bamboo_mosaic"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "oak_sapling"
        , custom_variants = Some [ stage_0_1 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "spruce_sapling"
        , custom_variants = Some [ stage_0_1 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "birch_sapling"
        , custom_variants = Some [ stage_0_1 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "jungle_sapling"
        , custom_variants = Some [ stage_0_1 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "acacia_sapling"
        , custom_variants = Some [ stage_0_1 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "cherry_sapling"
        , custom_variants = Some [ stage_0_1 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "dark_oak_sapling"
        , custom_variants = Some [ stage_0_1 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "mangrove_propagule"
        , custom_variants = Some [ stage_0_1, waterlogged ]
        , replacement_variants = Some [ intProp "age" 0 4, boolProp "hanging" ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { age = "0"
                    , hanging = "false"
                    , stage = "0"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , registerBasic "bedrock"
    , registerBasicLiquid "water"
    , registerBasicLiquidLight "lava" 15
    , registerBasic "sand"
    , registerBasic "suspicious_sand"
    , registerBasic "red_sand"
    , registerBasic "gravel"
    , registerBasic "suspicious_gravel"
    , registerBasic "gold_ore"
    , registerBasic "deepslate_gold_ore"
    , registerBasic "iron_ore"
    , registerBasic "deepslate_iron_ore"
    , registerBasic "coal_ore"
    , registerBasic "deepslate_coal_ore"
    , registerBasic "nether_gold_ore"
    , registerLog "oak_log"
    , registerLog "spruce_log"
    , registerLog "birch_log"
    , registerLog "jungle_log"
    , registerLog "acacia_log"
    , registerLog "cherry_log"
    , registerLog "dark_oak_log"
    , registerLog "mangrove_log"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "mangrove_roots"
        , custom_variants = Some [ waterlogged ]
        , default_override = Some (toNewMap (toMap { waterlogged = "false" }))
        }
    , registerLog "muddy_mangrove_roots"
    , registerLog "bamboo_block"
    , registerLog "stripped_spruce_log"
    , registerLog "stripped_birch_log"
    , registerLog "stripped_jungle_log"
    , registerLog "stripped_acacia_log"
    , registerLog "stripped_cherry_log"
    , registerLog "stripped_dark_oak_log"
    , registerLog "stripped_oak_log"
    , registerLog "stripped_mangrove_log"
    , registerLog "stripped_bamboo_block"
    , registerLog "oak_wood"
    , registerLog "spruce_wood"
    , registerLog "birch_wood"
    , registerLog "jungle_wood"
    , registerLog "acacia_wood"
    , registerLog "cherry_wood"
    , registerLog "dark_oak_wood"
    , registerLog "mangrove_wood"
    , registerLog "stripped_oak_wood"
    , registerLog "stripped_spruce_wood"
    , registerLog "stripped_birch_wood"
    , registerLog "stripped_jungle_wood"
    , registerLog "stripped_acacia_wood"
    , registerLog "stripped_cherry_wood"
    , registerLog "stripped_dark_oak_wood"
    , registerLog "stripped_mangrove_wood"
    , registerLeaves "oak_leaves"
    , registerLeaves "spruce_leaves"
    , registerLeaves "birch_leaves"
    , registerLeaves "jungle_leaves"
    , registerLeaves "acacia_leaves"
    , registerLeaves "cherry_leaves"
    , registerLeaves "dark_oak_leaves"
    , registerLeaves "mangrove_leaves"
    , registerLeaves "azalea_leaves"
    , registerLeaves "flowering_azalea_leaves"
    , registerBasic "sponge"
    , registerBasic "wet_sponge"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "glass"
        , default_extra_info = BlockstateInfo::{ opacity = BlockOpacity.Glass }
        }
    , registerBasic "lapis_ore"
    , registerBasic "deepslate_lapis_ore"
    , registerBasic "lapis_block"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "dispenser"
        , custom_variants = Some [ boolProp "triggered" ]
        , replacement_variants = Some [ facing_neswud ]
        , default_override = Some
            (toNewMap (toMap { facing = "north", triggered = "false" }))
        }
    , registerBasic "sandstone"
    , registerBasic "chiseled_sandstone"
    , registerBasic "cut_sandstone"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "note_block"
        , custom_variants = Some
          [ enumProp
              "instrument"
              [ "harp"
              , "basedrum"
              , "snare"
              , "hat"
              , "bass"
              , "flute"
              , "bell"
              , "guitar"
              , "chime"
              , "xylophone"
              , "iron_xylophone"
              , "cow_bell"
              , "didgeridoo"
              , "bit"
              , "banjo"
              , "pling"
              , "zombie"
              , "skeleton"
              , "creeper"
              , "dragon"
              , "wither_skeleton"
              , "piglin"
              , "custom_head"
              ]
          , intProp "note" 0 24
          , powered
          ]
        , default_override = Some
            ( toNewMap
                (toMap { instrument = "harp", note = "0", powered = "false" })
            )
        }
    , registerBed "white_bed"
    , registerBed "orange_bed"
    , registerBed "magenta_bed"
    , registerBed "light_blue_bed"
    , registerBed "yellow_bed"
    , registerBed "lime_bed"
    , registerBed "pink_bed"
    , registerBed "gray_bed"
    , registerBed "light_gray_bed"
    , registerBed "cyan_bed"
    , registerBed "purple_bed"
    , registerBed "blue_bed"
    , registerBed "brown_bed"
    , registerBed "green_bed"
    , registerBed "red_bed"
    , registerBed "black_bed"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "powered_rail"
        , custom_variants = Some [ waterlogged ]
        , replacement_variants = Some
          [ powered
          , enumProp
              "shape"
              [ "north_south"
              , "east_west"
              , "ascending_east"
              , "ascending_west"
              , "ascending_north"
              , "ascending_south"
              ]
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { powered = "false"
                    , shape = "north_south"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "detector_rail"
        , custom_variants = Some [ waterlogged ]
        , replacement_variants = Some
          [ powered
          , enumProp
              "shape"
              [ "north_south"
              , "east_west"
              , "ascending_east"
              , "ascending_west"
              , "ascending_north"
              , "ascending_south"
              ]
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { powered = "false"
                    , shape = "north_south"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "sticky_piston"
        , replacement_variants = Some [ boolProp "extended", facing_neswud ]
        , default_override = Some
            (toNewMap (toMap { extended = "false", facing = "north" }))
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , opacity = Some BlockOpacity.Transparent
              , light_info = Some skyTransparentInfo
              , collision_info = Some (complexCollisionInfo [
                , makeAABB [ 0.0, 0.0, 0.25 ] [ 1.0, 1.0, 1.0 ]
                ])
              }
            , conditions = toNewMap (toMap { extended = "true" })
            }
          ]
        }
    , registerTransparentNoCollider "cobweb"
    , registerTransparentNoCollider "short_grass"
    , registerTransparentNoCollider "fern"
    , registerTransparentNoCollider "dead_bush"
    , registerTransparentNoCollider "seagrass"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "tall_seagrass"
        , replacement_variants = Some [ enumProp "half" [ "upper", "lower" ] ]
        , default_override = Some (toNewMap (toMap { half = "lower" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "piston"
        , replacement_variants = Some [ boolProp "extended", facing_neswud ]
        , default_override = Some
            (toNewMap (toMap { extended = "false", facing = "north" }))
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , opacity = Some BlockOpacity.Transparent
              , light_info = Some skyTransparentInfo
              , collision_info = Some (complexCollisionInfo [
                , makeAABB [ 0.0, 0.0, 0.25 ] [ 1.0, 1.0, 1.0 ]
                ])
              }
            , conditions = toNewMap (toMap { extended = "true" })
            }
          ]
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "piston_head"
        , replacement_variants = Some
          [ facing_neswud
          , boolProp "short"
          , enumProp "type" [ "normal", "sticky" ]
          ]
        , default_override = Some
            ( toNewMap
                (toMap { facing = "north", short = "false", type = "normal" })
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = complexCollisionInfo [
            -- Stem
            , makeAABB [ 0.375, 0.375, 0.25 ] [ 0.625, 0.625, 1.0 ]
            -- Head
            , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 1.0, 0.25 ]
            ]
          }
        }
    , registerBasic "white_wool"
    , registerBasic "orange_wool"
    , registerBasic "magenta_wool"
    , registerBasic "light_blue_wool"
    , registerBasic "yellow_wool"
    , registerBasic "lime_wool"
    , registerBasic "pink_wool"
    , registerBasic "gray_wool"
    , registerBasic "light_gray_wool"
    , registerBasic "cyan_wool"
    , registerBasic "purple_wool"
    , registerBasic "blue_wool"
    , registerBasic "brown_wool"
    , registerBasic "green_wool"
    , registerBasic "red_wool"
    , registerBasic "black_wool"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "moving_piston"
        , custom_variants = Some
          [ facing_neswud, enumProp "type" [ "normal", "sticky" ] ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerTransparentNoCollider "dandelion"
    , registerTransparentNoCollider "torchflower"
    , registerTransparentNoCollider "poppy"
    , registerTransparentNoCollider "blue_orchid"
    , registerTransparentNoCollider "allium"
    , registerTransparentNoCollider "azure_bluet"
    , registerTransparentNoCollider "red_tulip"
    , registerTransparentNoCollider "orange_tulip"
    , registerTransparentNoCollider "white_tulip"
    , registerTransparentNoCollider "pink_tulip"
    , registerTransparentNoCollider "oxeye_daisy"
    , registerTransparentNoCollider "cornflower"
    , registerTransparentNoCollider "wither_rose"
    , registerTransparentNoCollider "lily_of_the_valley"
    , registerTransparentLight "brown_mushroom" 1
    , registerTransparentNoCollider "red_mushroom"
    , registerBasic "gold_block"
    , registerBasic "iron_block"
    , registerBasic "bricks"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "tnt"
        , custom_variants = Some [ boolProp "unstable" ]
        , default_override = Some (toNewMap (toMap { unstable = "false" }))
        }
    , registerBasic "bookshelf"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "chiseled_bookshelf"
        , custom_variants = Some
          [ facing_nswe
          , boolProp "slot_0_occupied"
          , boolProp "slot_1_occupied"
          , boolProp "slot_2_occupied"
          , boolProp "slot_3_occupied"
          , boolProp "slot_4_occupied"
          , boolProp "slot_5_occupied"
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { facing = "north"
                    , slot_0_occupied = "false"
                    , slot_1_occupied = "false"
                    , slot_2_occupied = "false"
                    , slot_3_occupied = "false"
                    , slot_4_occupied = "false"
                    , slot_5_occupied = "false"
                    }
                )
            )
        }
    , registerBasic "mossy_cobblestone"
    , registerBasic "obsidian"
    , registerTransparentLightNoCollider "torch" 14
    , Registration.Standard
        StandardRegistration::{
        , identifier = "wall_torch"
        , replacement_variants = Some [ facing_nswe ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 14
            }
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "fire"
        , custom_variants = Some
          [ age_0_15
          , boolProp "east"
          , boolProp "north"
          , boolProp "south"
          , boolProp "up"
          , boolProp "west"
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { age = "0"
                    , east = "false"
                    , north = "false"
                    , south = "false"
                    , up = "false"
                    , west = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 15
            }
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "soul_fire"
        , custom_variants = Some ([] : List CustomProperty)
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 10
            }
          , collision_info = emptyCollisionInfo
          }
        }
    , registerTransparent "spawner"
    , registerStairs "oak_stairs"
    , registerChest "chest"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "redstone_wire"
        , custom_variants = Some
          [ enumProp "east" [ "up", "side", "none" ]
          , enumProp "north" [ "up", "side", "none" ]
          , intProp "power" 0 15
          , enumProp "south" [ "up", "side", "none" ]
          , enumProp "west" [ "up", "side", "none" ]
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { east = "none"
                    , north = "none"
                    , south = "none"
                    , west = "none"
                    , power = "0"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerBasic "diamond_ore"
    , registerBasic "deepslate_diamond_ore"
    , registerBasic "diamond_block"
    , registerBasic "crafting_table"
    , registerTransparentNoCollider "wheat"
    , registerTransparent "farmland"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "furnace"
        , replacement_variants = Some [ facing_nswe, lit ]
        , default_override = Some
            (toNewMap (toMap { facing = "north", lit = "false" }))
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Opaque
                , emission_level = 13
                }
              }
            , conditions = toNewMap (toMap { lit = "true" })
            }
          ]
        }
    , registerSign "oak_sign"
    , registerSign "spruce_sign"
    , registerSign "birch_sign"
    , registerSign "acacia_sign"
    , registerSign "cherry_sign"
    , registerSign "jungle_sign"
    , registerSign "dark_oak_sign"
    , registerSign "mangrove_sign"
    , registerSign "bamboo_sign"
    , registerDoor "oak_door"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "ladder"
        , custom_variants = Some [ waterlogged ]
        , replacement_variants = Some [ facing_nswe ]
        , default_override = Some
            (toNewMap (toMap { facing = "north", waterlogged = "false" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "rail"
        , custom_variants = Some [ waterlogged ]
        , replacement_variants = Some
          [ enumProp
              "shape"
              [ "north_south"
              , "east_west"
              , "ascending_east"
              , "ascending_west"
              , "ascending_north"
              , "ascending_south"
              , "south_east"
              , "south_west"
              , "north_west"
              , "north_east"
              ]
          ]
        , default_override = Some
            (toNewMap (toMap { shape = "north_south", waterlogged = "false" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerStairs "cobblestone_stairs"
    , registerWallSign "oak_wall_sign"
    , registerWallSign "spruce_wall_sign"
    , registerWallSign "birch_wall_sign"
    , registerWallSign "acacia_wall_sign"
    , registerWallSign "cherry_wall_sign"
    , registerWallSign "jungle_wall_sign"
    , registerWallSign "dark_oak_wall_sign"
    , registerWallSign "mangrove_wall_sign"
    , registerWallSign "bamboo_wall_sign"
    , registerHangingSign "oak_hanging_sign"
    , registerHangingSign "spruce_hanging_sign"
    , registerHangingSign "birch_hanging_sign"
    , registerHangingSign "acacia_hanging_sign"
    , registerHangingSign "cherry_hanging_sign"
    , registerHangingSign "jungle_hanging_sign"
    , registerHangingSign "dark_oak_hanging_sign"
    , registerHangingSign "crimson_hanging_sign"
    , registerHangingSign "warped_hanging_sign"
    , registerHangingSign "mangrove_hanging_sign"
    , registerHangingSign "bamboo_hanging_sign"
    , registerWallHangingSign "oak_wall_hanging_sign"
    , registerWallHangingSign "spruce_wall_hanging_sign"
    , registerWallHangingSign "birch_wall_hanging_sign"
    , registerWallHangingSign "acacia_wall_hanging_sign"
    , registerWallHangingSign "cherry_wall_hanging_sign"
    , registerWallHangingSign "jungle_wall_hanging_sign"
    , registerWallHangingSign "dark_oak_wall_hanging_sign"
    , registerWallHangingSign "mangrove_wall_hanging_sign"
    , registerWallHangingSign "crimson_wall_hanging_sign"
    , registerWallHangingSign "warped_wall_hanging_sign"
    , registerWallHangingSign "bamboo_wall_hanging_sign"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "lever"
        , replacement_variants = Some
          [ enumProp "face" [ "floor", "wall", "ceiling" ]
          , facing_nswe
          , powered
          ]
        , default_override = Some
            ( toNewMap
                (toMap { face = "wall", facing = "north", powered = "false" })
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , collision_info = emptyCollisionInfo
          }
        }
    , registerPressurePlate "stone_pressure_plate"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "iron_door"
        , custom_variants = Some [ powered ]
        , replacement_variants = Some
          [ facing_nswe
          , enumProp "half" [ "upper", "lower" ]
          , enumProp "hinge" [ "left", "right" ]
          , boolProp "open"
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { facing = "north"
                    , half = "lower"
                    , hinge = "left"
                    , open = "false"
                    , powered = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerPressurePlate "oak_pressure_plate"
    , registerPressurePlate "spruce_pressure_plate"
    , registerPressurePlate "birch_pressure_plate"
    , registerPressurePlate "jungle_pressure_plate"
    , registerPressurePlate "acacia_pressure_plate"
    , registerPressurePlate "cherry_pressure_plate"
    , registerPressurePlate "dark_oak_pressure_plate"
    , registerPressurePlate "mangrove_pressure_plate"
    , registerPressurePlate "bamboo_pressure_plate"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "redstone_ore"
        , custom_variants = Some [ lit ]
        , default_override = Some (toNewMap (toMap { lit = "false" }))
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Opaque
                , emission_level = 15
                }
              }
            , conditions = toNewMap (toMap { lit = "true" })
            }
          ]
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "deepslate_redstone_ore"
        , custom_variants = Some [ lit ]
        , default_override = Some (toNewMap (toMap { lit = "false" }))
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Opaque
                , emission_level = 15
                }
              }
            , conditions = toNewMap (toMap { lit = "true" })
            }
          ]
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "redstone_torch"
        , replacement_variants = Some [ lit ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , collision_info = emptyCollisionInfo
          }
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Opaque
                , emission_level = 7
                }
              }
            , conditions = toNewMap (toMap { lit = "true" })
            }
          ]
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "redstone_wall_torch"
        , replacement_variants = Some [ facing_nswe, lit ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , registerButton "stone_button"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "snow"
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , collision_info = Some emptyCollisionInfo
              }
            , conditions = toNewMap (toMap { layers = "1" })
            }
          , { modifier = BlockstateInfoModifier::{
              , collision_info = Some (complexCollisionInfo [
                , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.125, 1.0 ]
                ])
              }
            , conditions = toNewMap (toMap { layers = "2" })
            }
          , { modifier = BlockstateInfoModifier::{
              , collision_info = Some (complexCollisionInfo [
                , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.25, 1.0 ]
                ])
              }
            , conditions = toNewMap (toMap { layers = "3" })
            }
          , { modifier = BlockstateInfoModifier::{
              , collision_info = Some (complexCollisionInfo [
                , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.375, 1.0 ]
                ])
              }
            , conditions = toNewMap (toMap { layers = "4" })
            }
          , { modifier = BlockstateInfoModifier::{
              , collision_info = Some (complexCollisionInfo [
                , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.5, 1.0 ]
                ])
              }
            , conditions = toNewMap (toMap { layers = "5" })
            }
          , { modifier = BlockstateInfoModifier::{
              , collision_info = Some (complexCollisionInfo [
                , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.625, 1.0 ]
                ])
              }
            , conditions = toNewMap (toMap { layers = "6" })
            }
          , { modifier = BlockstateInfoModifier::{
              , collision_info = Some (complexCollisionInfo [
                , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.75, 1.0 ]
                ])
              }
            , conditions = toNewMap (toMap { layers = "7" })
            }
          , { modifier = BlockstateInfoModifier::{
              , collision_info = Some (complexCollisionInfo [
                , makeAABB [ 0.0, 0.0, 0.0 ] [ 1.0, 0.875, 1.0 ]
                ])
              }
            , conditions = toNewMap (toMap { layers = "8" })
            }
          ]
        }
    , registerTransparent "ice"
    , registerBasic "snow_block"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "cactus"
        , custom_variants = Some [ age_0_15 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerBasic "clay"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "sugar_cane"
        , custom_variants = Some [ age_0_15 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "jukebox"
        , custom_variants = Some [ boolProp "has_record" ]
        , default_override = Some (toNewMap (toMap { has_record = "false" }))
        }
    , registerFence "oak_fence"
    , registerBasic "netherrack"
    , registerTransparent "soul_sand"
    , registerBasic "soul_soil"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "basalt"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "polished_basalt"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , registerTransparentLightNoCollider "soul_torch" 10
    , Registration.Standard
        StandardRegistration::{
        , identifier = "soul_wall_torch"
        , replacement_variants = Some [ facing_nswe ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 10
            }
          , collision_info = emptyCollisionInfo
          }
        }
    , registerBasicLight "glowstone" 15
    , registerTransparentLight "nether_portal" 11
    , Registration.Standard
        StandardRegistration::{
        , identifier = "carved_pumpkin"
        , replacement_variants = Some [ facing_nswe ]
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "jack_o_lantern"
        , replacement_variants = Some [ facing_nswe ]
        , default_extra_info = BlockstateInfo::{
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Opaque
            , emission_level = 15
            }
          }
        }
    , registerTransparent "cake"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "repeater"
        , replacement_variants = Some
          [ intProp "delay" 1 4
          , facing_nswe
          , boolProp "locked"
          , boolProp "powered"
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { delay = "1"
                    , facing = "north"
                    , locked = "false"
                    , powered = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerStainedGlass "white_stained_glass"
    , registerStainedGlass "orange_stained_glass"
    , registerStainedGlass "magenta_stained_glass"
    , registerStainedGlass "light_blue_stained_glass"
    , registerStainedGlass "yellow_stained_glass"
    , registerStainedGlass "lime_stained_glass"
    , registerStainedGlass "pink_stained_glass"
    , registerStainedGlass "gray_stained_glass"
    , registerStainedGlass "light_gray_stained_glass"
    , registerStainedGlass "cyan_stained_glass"
    , registerStainedGlass "purple_stained_glass"
    , registerStainedGlass "blue_stained_glass"
    , registerStainedGlass "brown_stained_glass"
    , registerStainedGlass "green_stained_glass"
    , registerStainedGlass "red_stained_glass"
    , registerStainedGlass "black_stained_glass"
    , registerTrapdoor "oak_trapdoor"
    , registerTrapdoor "spruce_trapdoor"
    , registerTrapdoor "birch_trapdoor"
    , registerTrapdoor "jungle_trapdoor"
    , registerTrapdoor "acacia_trapdoor"
    , registerTrapdoor "cherry_trapdoor"
    , registerTrapdoor "dark_oak_trapdoor"
    , registerTrapdoor "mangrove_trapdoor"
    , registerTrapdoor "bamboo_trapdoor"
    , registerBasic "stone_bricks"
    , registerBasic "mossy_stone_bricks"
    , registerBasic "cracked_stone_bricks"
    , registerBasic "chiseled_stone_bricks"
    , registerBasic "packed_mud"
    , registerBasic "mud_bricks"
    , registerBasic "infested_stone"
    , registerBasic "infested_cobblestone"
    , registerBasic "infested_stone_bricks"
    , registerBasic "infested_mossy_stone_bricks"
    , registerBasic "infested_cracked_stone_bricks"
    , registerBasic "infested_chiseled_stone_bricks"
    , registerMushroomBlock "brown_mushroom_block"
    , registerMushroomBlock "red_mushroom_block"
    , registerMushroomBlock "mushroom_stem"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "iron_bars"
        , custom_variants = Some
          [ boolProp "east"
          , boolProp "north"
          , boolProp "south"
          , waterlogged
          , boolProp "west"
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { east = "false"
                    , north = "false"
                    , south = "false"
                    , west = "false"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "chain"
        , custom_variants = Some [ waterlogged ]
        , default_override = Some
            (toNewMap (toMap { axis = "y", waterlogged = "false" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "glass_pane"
        , custom_variants = Some
          [ boolProp "east"
          , boolProp "north"
          , boolProp "south"
          , waterlogged
          , boolProp "west"
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { east = "false"
                    , north = "false"
                    , south = "false"
                    , west = "false"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.GlassPane
          , light_info = skyTransparentInfo
          }
        }
    , registerBasic "pumpkin"
    , registerBasic "melon"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "attached_pumpkin_stem"
        , replacement_variants = Some [ facing_nswe ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "attached_melon_stem"
        , replacement_variants = Some [ facing_nswe ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerTransparent "pumpkin_stem"
    , registerTransparent "melon_stem"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "vine"
        , custom_variants = Some
          [ boolProp "east"
          , boolProp "north"
          , boolProp "south"
          , boolProp "up"
          , boolProp "west"
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { east = "false"
                    , north = "false"
                    , south = "false"
                    , up = "false"
                    , west = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "glow_lichen"
        , custom_variants = Some
          [ boolProp "down"
          , boolProp "east"
          , boolProp "north"
          , boolProp "south"
          , boolProp "up"
          , waterlogged
          , boolProp "west"
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { east = "false"
                    , north = "false"
                    , south = "false"
                    , west = "false"
                    , up = "false"
                    , down = "false"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 7
            }
          , collision_info = emptyCollisionInfo
          }
        }
    , registerFenceGate "oak_fence_gate"
    , registerStairs "brick_stairs"
    , registerStairs "stone_brick_stairs"
    , registerStairs "mud_brick_stairs"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "mycelium"
        , replacement_variants = Some [ boolProp "snowy" ]
        , default_override = Some (toNewMap (toMap { snowy = "false" }))
        }
    , registerTransparent "lily_pad"
    , registerBasic "nether_bricks"
    , registerFence "nether_brick_fence"
    , registerStairs "nether_brick_stairs"
    , registerTransparent "nether_wart"
    , registerTransparentLight "enchanting_table" 7
    , Registration.Standard
        StandardRegistration::{
        , identifier = "brewing_stand"
        , custom_variants = Some
          [ boolProp "has_bottle_0"
          , boolProp "has_bottle_1"
          , boolProp "has_bottle_2"
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { has_bottle_0 = "false"
                    , has_bottle_1 = "false"
                    , has_bottle_2 = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 1
            }
          }
        }
    , registerTransparent "cauldron"
    , registerTransparent "water_cauldron"
    , registerTransparentLight "lava_cauldron" 15
    , registerTransparent "powder_snow_cauldron"
    , registerBasicLight "end_portal" 15
    , Registration.Standard
        StandardRegistration::{
        , identifier = "end_portal_frame"
        , replacement_variants = Some [ boolProp "eye", facing_nswe ]
        , default_override = Some
            (toNewMap (toMap { eye = "false", facing = "north" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 1
            }
          }
        }
    , registerBasic "end_stone"
    , registerTransparentLight "dragon_egg" 1
    , Registration.Standard
        StandardRegistration::{
        , identifier = "redstone_lamp"
        , replacement_variants = Some [ lit ]
        , default_override = Some (toNewMap (toMap { lit = "false" }))
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Opaque
                , emission_level = 15
                }
              }
            , conditions = toNewMap (toMap { lit = "true" })
            }
          ]
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "cocoa"
        , replacement_variants = Some [ intProp "age" 0 2, facing_nswe ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerStairs "sandstone_stairs"
    , registerBasic "emerald_ore"
    , registerBasic "deepslate_emerald_ore"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "ender_chest"
        , custom_variants = Some [ facing_nswe, waterlogged ]
        , default_override = Some
            (toNewMap (toMap { facing = "north", waterlogged = "false" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 7
            }
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "tripwire_hook"
        , replacement_variants = Some
          [ boolProp "attached", facing_nswe, powered ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { attached = "false", facing = "north", powered = "false" }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.FullCustom
        FullCustomRegistration::{
        , identifier = "tripwire"
        , custom_variants =
          [ boolProp "attached"
          , boolProp "disarmed"
          , boolProp "east"
          , boolProp "north"
          , powered
          , boolProp "south"
          , boolProp "west"
          ]
        , skip_properties = [ "disarmed", "powered" ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { attached = "false"
                    , disarmed = "false"
                    , east = "false"
                    , north = "false"
                    , powered = "false"
                    , south = "false"
                    , west = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerBasic "emerald_block"
    , registerStairs "spruce_stairs"
    , registerStairs "birch_stairs"
    , registerStairs "jungle_stairs"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "command_block"
        , replacement_variants = Some [ boolProp "conditional", facing_neswud ]
        , default_override = Some
            (toNewMap (toMap { conditional = "false", facing = "north" }))
        }
    , registerTransparentLight "beacon" 15
    , registerWall "cobblestone_wall"
    , registerWall "mossy_cobblestone_wall"
    , registerTransparent "flower_pot"
    , registerTransparent "potted_torchflower"
    , registerTransparent "potted_oak_sapling"
    , registerTransparent "potted_spruce_sapling"
    , registerTransparent "potted_birch_sapling"
    , registerTransparent "potted_jungle_sapling"
    , registerTransparent "potted_acacia_sapling"
    , registerTransparent "potted_cherry_sapling"
    , registerTransparent "potted_dark_oak_sapling"
    , registerTransparent "potted_mangrove_propagule"
    , registerTransparent "potted_fern"
    , registerTransparent "potted_dandelion"
    , registerTransparent "potted_poppy"
    , registerTransparent "potted_blue_orchid"
    , registerTransparent "potted_allium"
    , registerTransparent "potted_azure_bluet"
    , registerTransparent "potted_red_tulip"
    , registerTransparent "potted_orange_tulip"
    , registerTransparent "potted_white_tulip"
    , registerTransparent "potted_pink_tulip"
    , registerTransparent "potted_oxeye_daisy"
    , registerTransparent "potted_cornflower"
    , registerTransparent "potted_lily_of_the_valley"
    , registerTransparent "potted_wither_rose"
    , registerTransparent "potted_red_mushroom"
    , registerTransparent "potted_brown_mushroom"
    , registerTransparent "potted_dead_bush"
    , registerTransparent "potted_cactus"
    , registerTransparent "carrots"
    , registerTransparent "potatoes"
    , registerButton "oak_button"
    , registerButton "spruce_button"
    , registerButton "birch_button"
    , registerButton "jungle_button"
    , registerButton "acacia_button"
    , registerButton "cherry_button"
    , registerButton "dark_oak_button"
    , registerButton "mangrove_button"
    , registerButton "bamboo_button"
    , registerHead "skeleton_skull"
    , registerWallHead "skeleton_wall_skull"
    , registerHead "wither_skeleton_skull"
    , registerWallHead "wither_skeleton_wall_skull"
    , registerHead "zombie_head"
    , registerWallHead "zombie_wall_head"
    , registerHead "player_head"
    , registerWallHead "player_wall_head"
    , registerHead "creeper_head"
    , registerWallHead "creeper_wall_head"
    , registerHead "dragon_head"
    , registerWallHead "dragon_wall_head"
    , registerHead "piglin_head"
    , registerWallHead "piglin_wall_head"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "anvil"
        , replacement_variants = Some [ facing_nswe ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "chipped_anvil"
        , replacement_variants = Some [ facing_nswe ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "damaged_anvil"
        , replacement_variants = Some [ facing_nswe ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerChest "trapped_chest"
    , registerWeightedPressurePlate "light_weighted_pressure_plate"
    , registerWeightedPressurePlate "heavy_weighted_pressure_plate"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "comparator"
        , replacement_variants = Some
          [ facing_nswe, enumProp "mode" [ "compare", "subtract" ], powered ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { facing = "north", mode = "compare", powered = "false" }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "daylight_detector"
        , custom_variants = Some [ intProp "power" 0 15 ]
        , replacement_variants = Some [ boolProp "inverted" ]
        , default_override = Some
            (toNewMap (toMap { inverted = "false", power = "0" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerBasic "redstone_block"
    , registerBasic "nether_quartz_ore"
    , Registration.FullCustom
        FullCustomRegistration::{
        , identifier = "hopper"
        , custom_variants =
          [ boolProp "enabled"
          , enumProp "facing" [ "down", "north", "south", "west", "east" ]
          ]
        , skip_properties = [ "enabled" ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerBasic "quartz_block"
    , registerBasic "chiseled_quartz_block"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "quartz_pillar"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "quartz_stairs"
        , custom_variants = Some [ waterlogged ]
        , replacement_variants = Some
          [ facing_nswe
          , enumProp "half" [ "top", "bottom" ]
          , enumProp
              "shape"
              [ "straight"
              , "inner_left"
              , "inner_right"
              , "outer_left"
              , "outer_right"
              ]
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { facing = "north"
                    , half = "bottom"
                    , shape = "straight"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "activator_rail"
        , custom_variants = Some [ waterlogged ]
        , replacement_variants = Some
          [ powered
          , enumProp
              "shape"
              [ "north_south"
              , "east_west"
              , "ascending_east"
              , "ascending_west"
              , "ascending_north"
              , "ascending_south"
              ]
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { powered = "false"
                    , shape = "north_south"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "dropper"
        , custom_variants = Some [ boolProp "triggered" ]
        , replacement_variants = Some [ facing_neswud ]
        , default_override = Some
            (toNewMap (toMap { facing = "north", triggered = "false" }))
        }
    , registerBasic "white_terracotta"
    , registerBasic "orange_terracotta"
    , registerBasic "magenta_terracotta"
    , registerBasic "light_blue_terracotta"
    , registerBasic "yellow_terracotta"
    , registerBasic "lime_terracotta"
    , registerBasic "pink_terracotta"
    , registerBasic "gray_terracotta"
    , registerBasic "light_gray_terracotta"
    , registerBasic "cyan_terracotta"
    , registerBasic "purple_terracotta"
    , registerBasic "blue_terracotta"
    , registerBasic "brown_terracotta"
    , registerBasic "green_terracotta"
    , registerBasic "red_terracotta"
    , registerBasic "black_terracotta"
    , registerGlassPane "white_stained_glass_pane"
    , registerGlassPane "orange_stained_glass_pane"
    , registerGlassPane "magenta_stained_glass_pane"
    , registerGlassPane "light_blue_stained_glass_pane"
    , registerGlassPane "yellow_stained_glass_pane"
    , registerGlassPane "lime_stained_glass_pane"
    , registerGlassPane "pink_stained_glass_pane"
    , registerGlassPane "gray_stained_glass_pane"
    , registerGlassPane "light_gray_stained_glass_pane"
    , registerGlassPane "cyan_stained_glass_pane"
    , registerGlassPane "purple_stained_glass_pane"
    , registerGlassPane "blue_stained_glass_pane"
    , registerGlassPane "brown_stained_glass_pane"
    , registerGlassPane "green_stained_glass_pane"
    , registerGlassPane "red_stained_glass_pane"
    , registerGlassPane "black_stained_glass_pane"
    , registerStairs "acacia_stairs"
    , registerStairs "cherry_stairs"
    , registerStairs "dark_oak_stairs"
    , registerStairs "mangrove_stairs"
    , registerStairs "bamboo_stairs"
    , registerStairs "bamboo_mosaic_stairs"
    , registerTransparent "slime_block"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "barrier"
        , custom_variants = Some [ waterlogged ]
        , default_override = Some (toNewMap (toMap { waterlogged = "false" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "light"
        , custom_variants = Some [ waterlogged ]
        , replacement_variants = Some [ intProp "level" 0 15 ]
        , default_override = Some
            (toNewMap (toMap { level = "15", waterlogged = "false" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , light_info = Some skyTransparentInfo
              }
            , conditions = toNewMap (toMap { level = "0" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 1
                }
              }
            , conditions = toNewMap (toMap { level = "1" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 2
                }
              }
            , conditions = toNewMap (toMap { level = "2" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 3
                }
              }
            , conditions = toNewMap (toMap { level = "3" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 4
                }
              }
            , conditions = toNewMap (toMap { level = "4" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 5
                }
              }
            , conditions = toNewMap (toMap { level = "5" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 6
                }
              }
            , conditions = toNewMap (toMap { level = "6" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 7
                }
              }
            , conditions = toNewMap (toMap { level = "7" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 8
                }
              }
            , conditions = toNewMap (toMap { level = "8" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 9
                }
              }
            , conditions = toNewMap (toMap { level = "9" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 10
                }
              }
            , conditions = toNewMap (toMap { level = "10" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 11
                }
              }
            , conditions = toNewMap (toMap { level = "11" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 12
                }
              }
            , conditions = toNewMap (toMap { level = "12" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 13
                }
              }
            , conditions = toNewMap (toMap { level = "13" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 14
                }
              }
            , conditions = toNewMap (toMap { level = "14" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 15
                }
              }
            , conditions = toNewMap (toMap { level = "15" })
            }
          ]
        }
    , registerTrapdoor "iron_trapdoor"
    , registerBasic "prismarine"
    , registerBasic "prismarine_bricks"
    , registerBasic "dark_prismarine"
    , registerStairs "prismarine_stairs"
    , registerStairs "prismarine_brick_stairs"
    , registerStairs "dark_prismarine_stairs"
    , registerSlab "prismarine_slab"
    , registerSlab "prismarine_brick_slab"
    , registerSlab "dark_prismarine_slab"
    , registerTransparentLight "sea_lantern" 15
    , Registration.Standard
        StandardRegistration::{
        , identifier = "hay_block"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , registerTransparent "white_carpet"
    , registerTransparent "orange_carpet"
    , registerTransparent "magenta_carpet"
    , registerTransparent "light_blue_carpet"
    , registerTransparent "yellow_carpet"
    , registerTransparent "lime_carpet"
    , registerTransparent "pink_carpet"
    , registerTransparent "gray_carpet"
    , registerTransparent "light_gray_carpet"
    , registerTransparent "cyan_carpet"
    , registerTransparent "purple_carpet"
    , registerTransparent "blue_carpet"
    , registerTransparent "brown_carpet"
    , registerTransparent "green_carpet"
    , registerTransparent "red_carpet"
    , registerTransparent "black_carpet"
    , registerBasic "terracotta"
    , registerBasic "coal_block"
    , registerBasic "packed_ice"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "sunflower"
        , replacement_variants = Some [ enumProp "half" [ "upper", "lower" ] ]
        , default_override = Some (toNewMap (toMap { half = "lower" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "lilac"
        , replacement_variants = Some [ enumProp "half" [ "upper", "lower" ] ]
        , default_override = Some (toNewMap (toMap { half = "lower" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "rose_bush"
        , replacement_variants = Some [ enumProp "half" [ "upper", "lower" ] ]
        , default_override = Some (toNewMap (toMap { half = "lower" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "peony"
        , replacement_variants = Some [ enumProp "half" [ "upper", "lower" ] ]
        , default_override = Some (toNewMap (toMap { half = "lower" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "tall_grass"
        , replacement_variants = Some [ enumProp "half" [ "upper", "lower" ] ]
        , default_override = Some (toNewMap (toMap { half = "lower" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "large_fern"
        , replacement_variants = Some [ enumProp "half" [ "upper", "lower" ] ]
        , default_override = Some (toNewMap (toMap { half = "lower" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , registerBanner "white_banner"
    , registerBanner "orange_banner"
    , registerBanner "magenta_banner"
    , registerBanner "light_blue_banner"
    , registerBanner "yellow_banner"
    , registerBanner "lime_banner"
    , registerBanner "pink_banner"
    , registerBanner "gray_banner"
    , registerBanner "light_gray_banner"
    , registerBanner "cyan_banner"
    , registerBanner "purple_banner"
    , registerBanner "blue_banner"
    , registerBanner "brown_banner"
    , registerBanner "green_banner"
    , registerBanner "red_banner"
    , registerBanner "black_banner"
    , registerWallBanner "white_wall_banner"
    , registerWallBanner "orange_wall_banner"
    , registerWallBanner "magenta_wall_banner"
    , registerWallBanner "light_blue_wall_banner"
    , registerWallBanner "yellow_wall_banner"
    , registerWallBanner "lime_wall_banner"
    , registerWallBanner "pink_wall_banner"
    , registerWallBanner "gray_wall_banner"
    , registerWallBanner "light_gray_wall_banner"
    , registerWallBanner "cyan_wall_banner"
    , registerWallBanner "purple_wall_banner"
    , registerWallBanner "blue_wall_banner"
    , registerWallBanner "brown_wall_banner"
    , registerWallBanner "green_wall_banner"
    , registerWallBanner "red_wall_banner"
    , registerWallBanner "black_wall_banner"
    , registerBasic "red_sandstone"
    , registerBasic "chiseled_red_sandstone"
    , registerBasic "cut_red_sandstone"
    , registerStairs "red_sandstone_stairs"
    , registerSlab "oak_slab"
    , registerSlab "spruce_slab"
    , registerSlab "birch_slab"
    , registerSlab "jungle_slab"
    , registerSlab "acacia_slab"
    , registerSlab "cherry_slab"
    , registerSlab "dark_oak_slab"
    , registerSlab "mangrove_slab"
    , registerSlab "bamboo_slab"
    , registerSlab "bamboo_mosaic_slab"
    , registerSlab "stone_slab"
    , registerSlab "smooth_stone_slab"
    , registerSlab "sandstone_slab"
    , registerSlab "cut_sandstone_slab"
    , registerSlab "petrified_oak_slab"
    , registerSlab "cobblestone_slab"
    , registerSlab "brick_slab"
    , registerSlab "stone_brick_slab"
    , registerSlab "mud_brick_slab"
    , registerSlab "nether_brick_slab"
    , registerSlab "quartz_slab"
    , registerSlab "red_sandstone_slab"
    , registerSlab "cut_red_sandstone_slab"
    , registerSlab "purpur_slab"
    , registerBasic "smooth_stone"
    , registerBasic "smooth_sandstone"
    , registerBasic "smooth_quartz"
    , registerBasic "smooth_red_sandstone"
    , registerFenceGate "spruce_fence_gate"
    , registerFenceGate "birch_fence_gate"
    , registerFenceGate "jungle_fence_gate"
    , registerFenceGate "acacia_fence_gate"
    , registerFenceGate "cherry_fence_gate"
    , registerFenceGate "dark_oak_fence_gate"
    , registerFenceGate "mangrove_fence_gate"
    , registerFenceGate "bamboo_fence_gate"
    , registerFence "spruce_fence"
    , registerFence "birch_fence"
    , registerFence "jungle_fence"
    , registerFence "acacia_fence"
    , registerFence "cherry_fence"
    , registerFence "dark_oak_fence"
    , registerFence "mangrove_fence"
    , registerFence "bamboo_fence"
    , registerDoor "spruce_door"
    , registerDoor "birch_door"
    , registerDoor "jungle_door"
    , registerDoor "acacia_door"
    , registerDoor "cherry_door"
    , registerDoor "dark_oak_door"
    , registerDoor "mangrove_door"
    , registerDoor "bamboo_door"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "end_rod"
        , replacement_variants = Some [ facing_neswud ]
        , default_override = Some (toNewMap (toMap { facing = "up" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 14
            }
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "chorus_plant"
        , custom_variants = Some
          [ boolProp "down"
          , boolProp "east"
          , boolProp "north"
          , boolProp "south"
          , boolProp "up"
          , boolProp "west"
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { down = "false"
                    , east = "false"
                    , north = "false"
                    , south = "false"
                    , up = "false"
                    , west = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , registerTransparent "chorus_flower"
    , registerBasic "purpur_block"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "purpur_pillar"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , registerStairs "purpur_stairs"
    , registerBasic "end_stone_bricks"
    , registerTransparent "torchflower_crop"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "pitcher_crop"
        , replacement_variants = Some
          [ intProp "age" 0 4, enumProp "half" [ "upper", "lower" ] ]
        , default_override = Some
            (toNewMap (toMap { age = "0", half = "lower" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "pitcher_plant"
        , replacement_variants = Some [ enumProp "half" [ "upper", "lower" ] ]
        , default_override = Some (toNewMap (toMap { half = "lower" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , registerTransparent "beetroots"
    , registerTransparent "dirt_path"
    , registerBasicLight "end_gateway" 15
    , Registration.Standard
        StandardRegistration::{
        , identifier = "repeating_command_block"
        , replacement_variants = Some [ boolProp "conditional", facing_neswud ]
        , default_override = Some
            (toNewMap (toMap { conditional = "false", facing = "north" }))
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "chain_command_block"
        , replacement_variants = Some [ boolProp "conditional", facing_neswud ]
        , default_override = Some
            (toNewMap (toMap { conditional = "false", facing = "north" }))
        }
    , registerBasic "frosted_ice"
    , registerBasicLight "magma_block" 3
    , registerBasic "nether_wart_block"
    , registerBasic "red_nether_bricks"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "bone_block"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , registerTransparent "structure_void"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "observer"
        , replacement_variants = Some [ facing_neswud, powered ]
        , default_override = Some
            (toNewMap (toMap { facing = "south", powered = "false" }))
        }
    , registerShulkerBox "shulker_box"
    , registerShulkerBox "white_shulker_box"
    , registerShulkerBox "orange_shulker_box"
    , registerShulkerBox "magenta_shulker_box"
    , registerShulkerBox "light_blue_shulker_box"
    , registerShulkerBox "yellow_shulker_box"
    , registerShulkerBox "lime_shulker_box"
    , registerShulkerBox "pink_shulker_box"
    , registerShulkerBox "gray_shulker_box"
    , registerShulkerBox "light_gray_shulker_box"
    , registerShulkerBox "cyan_shulker_box"
    , registerShulkerBox "purple_shulker_box"
    , registerShulkerBox "blue_shulker_box"
    , registerShulkerBox "brown_shulker_box"
    , registerShulkerBox "green_shulker_box"
    , registerShulkerBox "red_shulker_box"
    , registerShulkerBox "black_shulker_box"
    , registerGlazedTerracotta "white_glazed_terracotta"
    , registerGlazedTerracotta "orange_glazed_terracotta"
    , registerGlazedTerracotta "magenta_glazed_terracotta"
    , registerGlazedTerracotta "light_blue_glazed_terracotta"
    , registerGlazedTerracotta "yellow_glazed_terracotta"
    , registerGlazedTerracotta "lime_glazed_terracotta"
    , registerGlazedTerracotta "pink_glazed_terracotta"
    , registerGlazedTerracotta "gray_glazed_terracotta"
    , registerGlazedTerracotta "light_gray_glazed_terracotta"
    , registerGlazedTerracotta "cyan_glazed_terracotta"
    , registerGlazedTerracotta "purple_glazed_terracotta"
    , registerGlazedTerracotta "blue_glazed_terracotta"
    , registerGlazedTerracotta "brown_glazed_terracotta"
    , registerGlazedTerracotta "green_glazed_terracotta"
    , registerGlazedTerracotta "red_glazed_terracotta"
    , registerGlazedTerracotta "black_glazed_terracotta"
    , registerBasic "white_concrete"
    , registerBasic "orange_concrete"
    , registerBasic "magenta_concrete"
    , registerBasic "light_blue_concrete"
    , registerBasic "yellow_concrete"
    , registerBasic "lime_concrete"
    , registerBasic "pink_concrete"
    , registerBasic "gray_concrete"
    , registerBasic "light_gray_concrete"
    , registerBasic "cyan_concrete"
    , registerBasic "purple_concrete"
    , registerBasic "blue_concrete"
    , registerBasic "brown_concrete"
    , registerBasic "green_concrete"
    , registerBasic "red_concrete"
    , registerBasic "black_concrete"
    , registerBasic "white_concrete_powder"
    , registerBasic "orange_concrete_powder"
    , registerBasic "magenta_concrete_powder"
    , registerBasic "light_blue_concrete_powder"
    , registerBasic "yellow_concrete_powder"
    , registerBasic "lime_concrete_powder"
    , registerBasic "pink_concrete_powder"
    , registerBasic "gray_concrete_powder"
    , registerBasic "light_gray_concrete_powder"
    , registerBasic "cyan_concrete_powder"
    , registerBasic "purple_concrete_powder"
    , registerBasic "blue_concrete_powder"
    , registerBasic "brown_concrete_powder"
    , registerBasic "green_concrete_powder"
    , registerBasic "red_concrete_powder"
    , registerBasic "black_concrete_powder"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "kelp"
        , custom_variants = Some [ intProp "age" 0 25 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerTransparentNoCollider "kelp_plant"
    , registerBasic "dried_kelp_block"
    , registerTransparent "turtle_egg"
    , registerTransparent "sniffer_egg"
    , registerBasic "dead_tube_coral_block"
    , registerBasic "dead_brain_coral_block"
    , registerBasic "dead_bubble_coral_block"
    , registerBasic "dead_fire_coral_block"
    , registerBasic "dead_horn_coral_block"
    , registerBasic "tube_coral_block"
    , registerBasic "brain_coral_block"
    , registerBasic "bubble_coral_block"
    , registerBasic "fire_coral_block"
    , registerBasic "horn_coral_block"
    , registerCoral "dead_tube_coral"
    , registerCoral "dead_brain_coral"
    , registerCoral "dead_bubble_coral"
    , registerCoral "dead_fire_coral"
    , registerCoral "dead_horn_coral"
    , registerCoral "tube_coral"
    , registerCoral "brain_coral"
    , registerCoral "bubble_coral"
    , registerCoral "fire_coral"
    , registerCoral "horn_coral"
    , registerCoral "dead_tube_coral_fan"
    , registerCoral "dead_brain_coral_fan"
    , registerCoral "dead_bubble_coral_fan"
    , registerCoral "dead_fire_coral_fan"
    , registerCoral "dead_horn_coral_fan"
    , registerCoral "tube_coral_fan"
    , registerCoral "brain_coral_fan"
    , registerCoral "bubble_coral_fan"
    , registerCoral "fire_coral_fan"
    , registerCoral "horn_coral_fan"
    , registerCoralWallFan "dead_tube_coral_wall_fan"
    , registerCoralWallFan "dead_brain_coral_wall_fan"
    , registerCoralWallFan "dead_bubble_coral_wall_fan"
    , registerCoralWallFan "dead_fire_coral_wall_fan"
    , registerCoralWallFan "dead_horn_coral_wall_fan"
    , registerCoralWallFan "tube_coral_wall_fan"
    , registerCoralWallFan "brain_coral_wall_fan"
    , registerCoralWallFan "bubble_coral_wall_fan"
    , registerCoralWallFan "fire_coral_wall_fan"
    , registerCoralWallFan "horn_coral_wall_fan"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "sea_pickle"
        , replacement_variants = Some
          [ intProp "pickles" 1 4, boolProp "waterlogged" ]
        , default_override = Some
            (toNewMap (toMap { pickles = "1", waterlogged = "true" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 15
            }
          }
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 15
                }
              }
            , conditions = toNewMap (toMap { pickles = "4", waterlogged = "true" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 12
                }
              }
            , conditions = toNewMap (toMap { pickles = "3", waterlogged = "true" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 9
                }
              }
            , conditions = toNewMap (toMap { pickles = "2", waterlogged = "true" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 6
                }
              }
            , conditions = toNewMap (toMap { pickles = "1", waterlogged = "true" })
            }
          ]
        }
    , registerBasic "blue_ice"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "conduit"
        , custom_variants = Some [ waterlogged ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 15
            }
          }
        }
    , registerTransparent "bamboo_sapling"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "bamboo"
        , custom_variants = Some
          [ intProp "age" 0 1
          , enumProp "leaves" [ "none", "small", "large" ]
          , intProp "stage" 0 1
          ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerTransparent "potted_bamboo"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "void_air"
        , properties = Properties::{ air_like = True }
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "cave_air"
        , properties = Properties::{ air_like = True }
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "bubble_column"
        , custom_variants = Some [ boolProp "drag" ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerStairs "polished_granite_stairs"
    , registerStairs "smooth_red_sandstone_stairs"
    , registerStairs "mossy_stone_brick_stairs"
    , registerStairs "polished_diorite_stairs"
    , registerStairs "mossy_cobblestone_stairs"
    , registerStairs "end_stone_brick_stairs"
    , registerStairs "stone_stairs"
    , registerStairs "smooth_sandstone_stairs"
    , registerStairs "smooth_quartz_stairs"
    , registerStairs "granite_stairs"
    , registerStairs "andesite_stairs"
    , registerStairs "red_nether_brick_stairs"
    , registerStairs "polished_andesite_stairs"
    , registerStairs "diorite_stairs"
    , registerSlab "polished_granite_slab"
    , registerSlab "smooth_red_sandstone_slab"
    , registerSlab "mossy_stone_brick_slab"
    , registerSlab "polished_diorite_slab"
    , registerSlab "mossy_cobblestone_slab"
    , registerSlab "end_stone_brick_slab"
    , registerSlab "smooth_sandstone_slab"
    , registerSlab "smooth_quartz_slab"
    , registerSlab "granite_slab"
    , registerSlab "andesite_slab"
    , registerSlab "red_nether_brick_slab"
    , registerSlab "polished_andesite_slab"
    , registerSlab "diorite_slab"
    , registerWall "brick_wall"
    , registerWall "prismarine_wall"
    , registerWall "red_sandstone_wall"
    , registerWall "mossy_stone_brick_wall"
    , registerWall "granite_wall"
    , registerWall "stone_brick_wall"
    , registerWall "mud_brick_wall"
    , registerWall "nether_brick_wall"
    , registerWall "andesite_wall"
    , registerWall "red_nether_brick_wall"
    , registerWall "sandstone_wall"
    , registerWall "end_stone_brick_wall"
    , registerWall "diorite_wall"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "scaffolding"
        , custom_variants = Some [ intProp "distance" 0 7, waterlogged ]
        , replacement_variants = Some [ boolProp "bottom" ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { bottom = "false", distance = "7", waterlogged = "false" }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "loom"
        , replacement_variants = Some [ facing_nswe ]
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "barrel"
        , replacement_variants = Some [ facing_neswud, boolProp "open" ]
        , default_override = Some
            (toNewMap (toMap { facing = "north", open = "false" }))
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "smoker"
        , replacement_variants = Some [ facing_nswe, lit ]
        , default_override = Some
            (toNewMap (toMap { facing = "north", lit = "false" }))
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Opaque
                , emission_level = 13
                }
              }
            , conditions = toNewMap (toMap { lit = "true" })
            }
          ]
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "blast_furnace"
        , replacement_variants = Some [ facing_nswe, lit ]
        , default_override = Some
            (toNewMap (toMap { facing = "north", lit = "false" }))
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Opaque
                , emission_level = 13
                }
              }
            , conditions = toNewMap (toMap { lit = "true" })
            }
          ]
        }
    , registerBasic "cartography_table"
    , registerBasic "fletching_table"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "grindstone"
        , replacement_variants = Some
          [ enumProp "face" [ "floor", "wall", "ceiling" ], facing_nswe ]
        , default_override = Some
            (toNewMap (toMap { face = "wall", facing = "north" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "lectern"
        , custom_variants = Some [ boolProp "has_book", powered ]
        , replacement_variants = Some [ facing_nswe ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { facing = "north", has_book = "false", powered = "false" }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerBasic "smithing_table"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "stonecutter"
        , replacement_variants = Some [ facing_nswe ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "bell"
        , custom_variants = Some [ powered ]
        , replacement_variants = Some
          [ enumProp
              "attachment"
              [ "floor", "ceiling", "single_wall", "double_wall" ]
          , facing_nswe
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { attachment = "floor"
                    , facing = "north"
                    , powered = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerLantern "lantern" 15
    , registerLantern "soul_lantern" 10
    , Registration.Standard
        StandardRegistration::{
        , identifier = "campfire"
        , custom_variants = Some [ boolProp "signal_fire", waterlogged ]
        , replacement_variants = Some [ facing_nswe, lit ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { facing = "north"
                    , lit = "true"
                    , signal_fire = "false"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Opaque
                , emission_level = 15
                }
              }
            , conditions = toNewMap (toMap { lit = "true" })
            }
          ]
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "soul_campfire"
        , custom_variants = Some [ boolProp "signal_fire", waterlogged ]
        , replacement_variants = Some [ facing_nswe, lit ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { facing = "north"
                    , lit = "true"
                    , signal_fire = "false"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Opaque
                , emission_level = 10
                }
              }
            , conditions = toNewMap (toMap { lit = "true" })
            }
          ]
        }
    , registerTransparentNoCollider "sweet_berry_bush"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "warped_stem"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "stripped_warped_stem"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "warped_hyphae"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "stripped_warped_hyphae"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , registerBasic "warped_nylium"
    , registerBasic "warped_fungus"
    , registerBasic "warped_wart_block"
    , registerBasic "warped_roots"
    , registerBasic "nether_sprouts"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "crimson_stem"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "stripped_crimson_stem"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "crimson_hyphae"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "stripped_crimson_hyphae"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , registerBasic "crimson_nylium"
    , registerBasic "crimson_fungus"
    , registerBasicLight "shroomlight" 15
    , Registration.Standard
        StandardRegistration::{
        , identifier = "weeping_vines"
        , custom_variants = Some [ age_0_25 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerTransparent "weeping_vines_plant"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "twisting_vines"
        , custom_variants = Some [ age_0_25 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerTransparent "twisting_vines_plant"
    , registerBasic "crimson_roots"
    , registerBasic "crimson_planks"
    , registerBasic "warped_planks"
    , registerSlab "crimson_slab"
    , registerSlab "warped_slab"
    , registerPressurePlate "crimson_pressure_plate"
    , registerPressurePlate "warped_pressure_plate"
    , registerFence "crimson_fence"
    , registerFence "warped_fence"
    , registerTrapdoor "crimson_trapdoor"
    , registerTrapdoor "warped_trapdoor"
    , registerFenceGate "crimson_fence_gate"
    , registerFenceGate "warped_fence_gate"
    , registerStairs "crimson_stairs"
    , registerStairs "warped_stairs"
    , registerButton "crimson_button"
    , registerButton "warped_button"
    , registerDoor "crimson_door"
    , registerDoor "warped_door"
    , registerSign "crimson_sign"
    , registerSign "warped_sign"
    , registerWallSign "crimson_wall_sign"
    , registerWallSign "warped_wall_sign"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "structure_block"
        , replacement_variants = Some
          [ enumProp "mode" [ "save", "load", "corner", "data" ] ]
        , default_override = Some (toNewMap (toMap { mode = "load" }))
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "jigsaw"
        , replacement_variants = Some
          [ enumProp
              "orientation"
              [ "down_east"
              , "down_north"
              , "down_south"
              , "down_west"
              , "up_east"
              , "up_north"
              , "up_south"
              , "up_west"
              , "west_up"
              , "east_up"
              , "north_up"
              , "south_up"
              ]
          ]
        , default_override = Some
            (toNewMap (toMap { orientation = "north_up" }))
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "composter"
        , custom_variants = Some [ intProp "level" 0 8 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "target"
        , custom_variants = Some [ intProp "power" 0 15 ]
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "bee_nest"
        , replacement_variants = Some [ facing_nswe, intProp "honey_level" 0 5 ]
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "beehive"
        , replacement_variants = Some [ facing_nswe, intProp "honey_level" 0 5 ]
        }
    , registerTransparent "honey_block"
    , registerBasic "honeycomb_block"
    , registerBasic "netherite_block"
    , registerBasic "ancient_debris"
    , registerBasicLight "crying_obsidian" 10
    , Registration.Standard
        StandardRegistration::{
        , identifier = "respawn_anchor"
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Opaque
                , emission_level = 15
                }
              }
            , conditions = toNewMap (toMap { charges = "4" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Opaque
                , emission_level = 11
                }
              }
            , conditions = toNewMap (toMap { charges = "3" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Opaque
                , emission_level = 7
                }
              }
            , conditions = toNewMap (toMap { charges = "2" })
            }
          , { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Opaque
                , emission_level = 3
                }
              }
            , conditions = toNewMap (toMap { charges = "1" })
            }
          ]
        }
    , registerTransparent "potted_crimson_fungus"
    , registerTransparent "potted_warped_fungus"
    , registerTransparent "potted_crimson_roots"
    , registerTransparent "potted_warped_roots"
    , registerBasic "lodestone"
    , registerBasic "blackstone"
    , registerStairs "blackstone_stairs"
    , registerWall "blackstone_wall"
    , registerSlab "blackstone_slab"
    , registerBasic "polished_blackstone"
    , registerBasic "polished_blackstone_bricks"
    , registerBasic "cracked_polished_blackstone_bricks"
    , registerBasic "chiseled_polished_blackstone"
    , registerSlab "polished_blackstone_brick_slab"
    , registerStairs "polished_blackstone_brick_stairs"
    , registerWall "polished_blackstone_brick_wall"
    , registerBasic "gilded_blackstone"
    , registerStairs "polished_blackstone_stairs"
    , registerSlab "polished_blackstone_slab"
    , registerPressurePlate "polished_blackstone_pressure_plate"
    , registerButton "polished_blackstone_button"
    , registerWall "polished_blackstone_wall"
    , registerBasic "chiseled_nether_bricks"
    , registerBasic "cracked_nether_bricks"
    , registerBasic "quartz_bricks"
    , registerCandle "candle"
    , registerCandle "white_candle"
    , registerCandle "orange_candle"
    , registerCandle "magenta_candle"
    , registerCandle "light_blue_candle"
    , registerCandle "yellow_candle"
    , registerCandle "lime_candle"
    , registerCandle "pink_candle"
    , registerCandle "gray_candle"
    , registerCandle "light_gray_candle"
    , registerCandle "cyan_candle"
    , registerCandle "purple_candle"
    , registerCandle "blue_candle"
    , registerCandle "brown_candle"
    , registerCandle "green_candle"
    , registerCandle "red_candle"
    , registerCandle "black_candle"
    , registerCandleCake "candle_cake"
    , registerCandleCake "white_candle_cake"
    , registerCandleCake "orange_candle_cake"
    , registerCandleCake "magenta_candle_cake"
    , registerCandleCake "light_blue_candle_cake"
    , registerCandleCake "yellow_candle_cake"
    , registerCandleCake "lime_candle_cake"
    , registerCandleCake "pink_candle_cake"
    , registerCandleCake "gray_candle_cake"
    , registerCandleCake "light_gray_candle_cake"
    , registerCandleCake "cyan_candle_cake"
    , registerCandleCake "purple_candle_cake"
    , registerCandleCake "blue_candle_cake"
    , registerCandleCake "brown_candle_cake"
    , registerCandleCake "green_candle_cake"
    , registerCandleCake "red_candle_cake"
    , registerCandleCake "black_candle_cake"
    , registerBasic "amethyst_block"
    , registerBasic "budding_amethyst"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "amethyst_cluster"
        , custom_variants = Some [ waterlogged ]
        , replacement_variants = Some [ facing_neswud ]
        , default_override = Some
            (toNewMap (toMap { facing = "up", waterlogged = "false" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 5
            }
          }
        }
    , registerAmethystBud "large_amethyst_bud" 5
    , registerAmethystBud "medium_amethyst_bud" 2
    , registerAmethystBud "small_amethyst_bud" 1
    , registerBasic "tuff"
    , registerSlab "tuff_slab"
    , registerStairs "tuff_stairs"
    , registerWall "tuff_wall"
    , registerBasic "polished_tuff"
    , registerSlab "polished_tuff_slab"
    , registerStairs "polished_tuff_stairs"
    , registerWall "polished_tuff_wall"
    , registerBasic "chiseled_tuff"
    , registerBasic "tuff_bricks"
    , registerSlab "tuff_brick_slab"
    , registerStairs "tuff_brick_stairs"
    , registerWall "tuff_brick_wall"
    , registerBasic "chiseled_tuff_bricks"
    , registerBasic "calcite"
    , registerTransparent "tinted_glass"
    , registerBasic "powder_snow"
    , Registration.FullCustom
        FullCustomRegistration::{
        , identifier = "sculk_sensor"
        , custom_variants =
          [ intProp "power" 0 15
          , enumProp "sculk_sensor_phase" [ "inactive", "active", "cooldown" ]
          , waterlogged
          ]
        , skip_properties = [ "power", "waterlogged" ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { power = "0"
                    , sculk_sensor_phase = "inactive"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Transparent
            , emission_level = 1
            }
          }
        }
    , Registration.FullCustom
        FullCustomRegistration::{
        , identifier = "calibrated_sculk_sensor"
        , custom_variants =
          [ facing_nswe
          , intProp "power" 0 15
          , enumProp "sculk_sensor_phase" [ "inactive", "active", "cooldown" ]
          , waterlogged
          ]
        , skip_properties = [ "power", "waterlogged" ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { facing = "north"
                    , power = "0"
                    , sculk_sensor_phase = "inactive"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerBasic "sculk"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "sculk_vein"
        , custom_variants = Some
          [ boolProp "down"
          , boolProp "east"
          , boolProp "north"
          , boolProp "south"
          , boolProp "up"
          , waterlogged
          , boolProp "west"
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { down = "false"
                    , east = "false"
                    , north = "false"
                    , south = "false"
                    , up = "false"
                    , west = "false"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          , collision_info = emptyCollisionInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "sculk_catalyst"
        , replacement_variants = Some [ boolProp "bloom" ]
        , default_override = Some (toNewMap (toMap { bloom = "false" }))
        , default_extra_info = BlockstateInfo::{
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Opaque
            , emission_level = 6
            }
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "sculk_shrieker"
        , custom_variants = Some [ boolProp "shrieking", waterlogged ]
        , replacement_variants = Some [ boolProp "can_summon" ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { can_summon = "false"
                    , shrieking = "false"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerBasic "copper_block"
    , registerBasic "exposed_copper"
    , registerBasic "weathered_copper"
    , registerBasic "oxidized_copper"
    , registerBasic "copper_ore"
    , registerBasic "deepslate_copper_ore"
    , registerBasic "oxidized_cut_copper"
    , registerBasic "weathered_cut_copper"
    , registerBasic "exposed_cut_copper"
    , registerBasic "cut_copper"
    , registerBasic "oxidized_chiseled_copper"
    , registerBasic "weathered_chiseled_copper"
    , registerBasic "exposed_chiseled_copper"
    , registerBasic "chiseled_copper"
    , registerBasic "waxed_oxidized_chiseled_copper"
    , registerBasic "waxed_weathered_chiseled_copper"
    , registerBasic "waxed_exposed_chiseled_copper"
    , registerBasic "waxed_chiseled_copper"
    , registerStairs "oxidized_cut_copper_stairs"
    , registerStairs "weathered_cut_copper_stairs"
    , registerStairs "exposed_cut_copper_stairs"
    , registerStairs "cut_copper_stairs"
    , registerSlab "oxidized_cut_copper_slab"
    , registerSlab "weathered_cut_copper_slab"
    , registerSlab "exposed_cut_copper_slab"
    , registerSlab "cut_copper_slab"
    , registerBasic "waxed_copper_block"
    , registerBasic "waxed_weathered_copper"
    , registerBasic "waxed_exposed_copper"
    , registerBasic "waxed_oxidized_copper"
    , registerBasic "waxed_oxidized_cut_copper"
    , registerBasic "waxed_weathered_cut_copper"
    , registerBasic "waxed_exposed_cut_copper"
    , registerBasic "waxed_cut_copper"
    , registerStairs "waxed_oxidized_cut_copper_stairs"
    , registerStairs "waxed_weathered_cut_copper_stairs"
    , registerStairs "waxed_exposed_cut_copper_stairs"
    , registerStairs "waxed_cut_copper_stairs"
    , registerSlab "waxed_oxidized_cut_copper_slab"
    , registerSlab "waxed_weathered_cut_copper_slab"
    , registerSlab "waxed_exposed_cut_copper_slab"
    , registerSlab "waxed_cut_copper_slab"
    , registerDoor "copper_door"
    , registerDoor "exposed_copper_door"
    , registerDoor "oxidized_copper_door"
    , registerDoor "weathered_copper_door"
    , registerDoor "waxed_copper_door"
    , registerDoor "waxed_exposed_copper_door"
    , registerDoor "waxed_oxidized_copper_door"
    , registerDoor "waxed_weathered_copper_door"
    , registerTrapdoor "copper_trapdoor"
    , registerTrapdoor "exposed_copper_trapdoor"
    , registerTrapdoor "oxidized_copper_trapdoor"
    , registerTrapdoor "weathered_copper_trapdoor"
    , registerTrapdoor "waxed_copper_trapdoor"
    , registerTrapdoor "waxed_exposed_copper_trapdoor"
    , registerTrapdoor "waxed_oxidized_copper_trapdoor"
    , registerTrapdoor "waxed_weathered_copper_trapdoor"
    , registerGrate "copper_grate"
    , registerGrate "exposed_copper_grate"
    , registerGrate "weathered_copper_grate"
    , registerGrate "oxidized_copper_grate"
    , registerGrate "waxed_copper_grate"
    , registerGrate "waxed_exposed_copper_grate"
    , registerGrate "waxed_weathered_copper_grate"
    , registerGrate "waxed_oxidized_copper_grate"
    , registerBulb "copper_bulb"
    , registerBulb "exposed_copper_bulb"
    , registerBulb "weathered_copper_bulb"
    , registerBulb "oxidized_copper_bulb"
    , registerBulb "waxed_copper_bulb"
    , registerBulb "waxed_exposed_copper_bulb"
    , registerBulb "waxed_weathered_copper_bulb"
    , registerBulb "waxed_oxidized_copper_bulb"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "lightning_rod"
        , custom_variants = Some [ waterlogged ]
        , replacement_variants = Some [ facing_neswud, powered ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { facing = "up", powered = "false", waterlogged = "false" }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "pointed_dripstone"
        , custom_variants = Some [ waterlogged ]
        , replacement_variants = Some
          [ enumProp
              "thickness"
              [ "tip_merge", "tip", "frustum", "middle", "base" ]
          , enumProp "vertical_direction" [ "up", "down" ]
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { thickness = "tip"
                    , vertical_direction = "up"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerBasic "dripstone_block"
    , Registration.FullCustom
        FullCustomRegistration::{
        , identifier = "cave_vines"
        , custom_variants = [ age_0_25, boolProp "berries" ]
        , skip_properties = [ "age" ]
        , default_override = Some
            (toNewMap (toMap { age = "0", berries = "false" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        , extra_info_modifiers =
          [ { modifier = BlockstateInfoModifier::{
              , light_info = Some {
                , sky_light_opacity = SkyLightOpacity.Transparent
                , emission_level = 14
                }
              }
            , conditions = toNewMap (toMap { berries = "true" })
            }
          ]
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "cave_vines_plant"
        , replacement_variants = Some [ boolProp "berries" ]
        , default_override = Some (toNewMap (toMap { berries = "false" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerTransparent "spore_blossom"
    , registerTransparent "azalea"
    , registerTransparent "flowering_azalea"
    , registerTransparent "moss_carpet"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "pink_petals"
        , custom_variants = Some [ facing_nswe, intProp "flower_amount" 1 4 ]
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , registerTransparent "moss_block"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "big_dripleaf"
        , custom_variants = Some [ waterlogged ]
        , replacement_variants = Some
          [ facing_nswe
          , enumProp "tilt" [ "none", "unstable", "partial", "full" ]
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { facing = "north", tilt = "none", waterlogged = "false" }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "big_dripleaf_stem"
        , custom_variants = Some [ waterlogged ]
        , replacement_variants = Some [ facing_nswe ]
        , default_override = Some
            (toNewMap (toMap { facing = "north", waterlogged = "false" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "small_dripleaf"
        , custom_variants = Some [ waterlogged ]
        , replacement_variants = Some
          [ facing_nswe, enumProp "half" [ "upper", "lower" ] ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { facing = "north", half = "lower", waterlogged = "false" }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "hanging_roots"
        , custom_variants = Some [ waterlogged ]
        , default_override = Some (toNewMap (toMap { waterlogged = "false" }))
        }
    , registerBasic "rooted_dirt"
    , registerBasic "mud"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "deepslate"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , registerBasic "cobbled_deepslate"
    , registerStairs "cobbled_deepslate_stairs"
    , registerSlab "cobbled_deepslate_slab"
    , registerWall "cobbled_deepslate_wall"
    , registerBasic "polished_deepslate"
    , registerStairs "polished_deepslate_stairs"
    , registerSlab "polished_deepslate_slab"
    , registerWall "polished_deepslate_wall"
    , registerBasic "deepslate_tiles"
    , registerStairs "deepslate_tile_stairs"
    , registerSlab "deepslate_tile_slab"
    , registerWall "deepslate_tile_wall"
    , registerBasic "deepslate_bricks"
    , registerStairs "deepslate_brick_stairs"
    , registerSlab "deepslate_brick_slab"
    , registerWall "deepslate_brick_wall"
    , registerBasic "chiseled_deepslate"
    , registerBasic "cracked_deepslate_bricks"
    , registerBasic "cracked_deepslate_tiles"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "infested_deepslate"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        }
    , registerBasic "smooth_basalt"
    , registerBasic "raw_iron_block"
    , registerBasic "raw_copper_block"
    , registerBasic "raw_gold_block"
    , registerTransparent "potted_azalea_bush"
    , registerTransparent "potted_flowering_azalea_bush"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "ochre_froglight"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        , default_extra_info = BlockstateInfo::{
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Opaque
            , emission_level = 15
            }
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "verdant_froglight"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        , default_extra_info = BlockstateInfo::{
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Opaque
            , emission_level = 15
            }
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "pearlescent_froglight"
        , default_override = Some (toNewMap (toMap { axis = "y" }))
        , default_extra_info = BlockstateInfo::{
          , light_info = {
            , sky_light_opacity = SkyLightOpacity.Opaque
            , emission_level = 15
            }
          }
        }
    , registerTransparent "frogspawn"
    , registerBasic "reinforced_deepslate"
    , Registration.Standard
        StandardRegistration::{
        , identifier = "decorated_pot"
        , custom_variants = Some
          [ boolProp "cracked", facing_nswe, waterlogged ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { cracked = "false"
                    , facing = "north"
                    , waterlogged = "false"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "crafter"
        , replacement_variants = Some
          [ boolProp "crafting"
          , enumProp
              "orientation"
              [ "down_east"
              , "down_north"
              , "down_south"
              , "down_west"
              , "up_east"
              , "up_north"
              , "up_south"
              , "up_west"
              , "west_up"
              , "east_up"
              , "north_up"
              , "south_up"
              ]
          , boolProp "triggered"
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { crafting = "false"
                    , orientation = "north_up"
                    , triggered = "false"
                    }
                )
            )
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "trial_spawner"
        , replacement_variants = Some
          [ boolProp "ominous"
          , enumProp
              "trial_spawner_state"
              [ "inactive"
              , "waiting_for_players"
              , "active"
              , "waiting_for_reward_ejection"
              , "ejecting_reward"
              , "cooldown"
              ]
          ]
        , default_override = Some
            ( toNewMap
                (toMap { ominous = "false", trial_spawner_state = "inactive" })
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "vault"
        , replacement_variants = Some
          [ facing_nswe
          , boolProp "ominous"
          , enumProp
              "vault_state"
              [ "inactive", "active", "unlocking", "ejecting" ]
          ]
        , default_override = Some
            ( toNewMap
                ( toMap
                    { facing = "north"
                    , ominous = "false"
                    , vault_state = "inactive"
                    }
                )
            )
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    , Registration.Standard
        StandardRegistration::{
        , identifier = "heavy_core"
        , custom_variants = Some [ waterlogged ]
        , default_override = Some (toNewMap (toMap { waterlogged = "false" }))
        , default_extra_info = BlockstateInfo::{
          , opacity = BlockOpacity.Transparent
          , light_info = skyTransparentInfo
          }
        }
    ]
