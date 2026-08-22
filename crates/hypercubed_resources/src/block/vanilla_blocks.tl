local serialise_object_table_raw

local serialise_array_table_raw

local function serialise_to_json_raw(v, pretty)
  if type(v) == 'nil' then
    return "null"
  elseif type(v) == 'number' then
    return tostring(v)
  elseif type(v) == 'boolean' then
    return tostring(v)
  elseif type(v) == 'string' then
    return string.format('%q', v)
  elseif type(v) == 'table' then
    local meta = getmetatable(v)
    if meta.__json_serialise then
      return meta.__json_serialise(v, pretty)
    elseif v.__table_type == 'object' then
      return serialise_object_table_raw(v, pretty)
    elseif v.__table_type == 'array' then
      return serialise_array_table_raw(v, pretty)
    elseif v[1] ~= nil then
      return serialise_array_table_raw(v, pretty)
    else
      return serialise_object_table_raw(v, pretty)
    end
  end
end

function serialise_object_table_raw(obj, pretty)
  local fields = {}
  for k, v in pairs(v) do
    local k_serialised = serialise_to_json_raw(k, false)
    local v_serialised = serialise_to_json_raw(v, pretty)
    table.insert(k_serialised .. ": " .. v_serialised)
  end
  if #fields == 0 then
    return "{}"
  elseif pretty then
    return "{\n  " .. table.concat(fields, ",\n  ") .. "\n}"
  else
    return "{" .. table.concat(fields, ",") .. "}"
  end
end

function serialise_array_table_raw(arr, pretty)
  local items = {}
  for _, v in ipairs(arr) do
    table.insert(serialise_to_json_raw(v, pretty))
  end
  if #items == 0 then
    return "[]"
  elseif pretty then
    return "[\n  " .. table.concat(items, ",\n  ") .. "\n]"
  else
    return "[" .. table.concat(items, ",") .. "]"
  end
end

local function json_serialise(v)
  serialise_to_json_raw(v, false)
end

local function json_serialise_pretty(v)
  serialise_to_json_raw(v, true)
end

local type Vec3 = {number, number, number}

local record CustomBooleanPropertyType
  Boolean: {nil}
end

local record CustomIntPropertyTypeData
  start: number
  ['end']: number
end

local record CustomIntPropertyType
  Int: CustomIntPropertyTypeData
end

local record CustomEnumPropertyType
  Enum: {string}
end

local type CustomPropertyType =
  CustomBooleanPropertyType
  | CustomIntPropertyType
  | CustomEnumPropertyType

local record Properties
  air_like: bool
end

local default_properties: Properties = { air_like = false }

local enum BlockOpacity
  'Opaque'
  'Leaves'
  'Glass'
  'GlassPane'
  'Transparent'
end

local enum SkyLightOpacity
  'Opaque'
  'Translucent'
  'Transparent'
end

local record AABB
  corner_1: Vec3
  corner_2: Vec3
end

local record BlockstateInfoModifier
  opacity: BlockOpacity | nil
end

local record DefaultOverride
  key: string
  value: string
end

local interface AnyRegistration
  type: string
  identifier: string
  properties: Properties
  default_extra_info: BlockstateInfo
  extra_info_modifiers: {BlockstateInfoModifierCase}
end

local record StandardRegistration is AnyRegistration
  where self.type == 'Standard'
  custom_variants: {CustomProperty} | nil
  replacement_variants: {CustomProperty} | nil
  default_override: DefaultOverride | nil
end

local type Registration = LiquidRegistration | StandardRegistration | FullCustomRegistration

local standard_registration
do
  local standard_registration_defaults = {
    properties = {},
    default_extra_info = {},
    extra_info_modifiers = {},
  }
  local standard_registration_meta: metatable<StandardRegistration> = {
    __index = standard_registration_defaults,
    __json_serialise = function(self, pretty)
      return serialise_object_table_raw(
        {
          type = 'Standard',
          identifier = self.identifier,
          custom_variants = self.custom_variants,
          replacement_variants = self.replacement_variants,
          default_override = self.default_override,
          properties = self.properties,
          default_extra_info = self.default_extra_info,
          extra_info_modifiers = self.extra_info_modifiers,
        },
        pretty
      )
    end,
  }
  function standard_registration(obj)
    return setmetatable(obj, standard_registration_meta)
  end
end
