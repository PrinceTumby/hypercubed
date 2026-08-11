local serialise_object_table_raw

local serialise_array_table_raw

local function serialise_to_json_raw(v, pretty)
  if type(v) == 'nil' then
    return "null"
  else if type(v) == 'number' then
    return tostring(v)
  else if type(v) == 'boolean' then
    return tostring(v)
  else if type(v) == 'string' then
    return string.format('%q', v)
  else if type(v) == 'table' then
    local meta = getmetatable(v)
    if meta.__json_serialise then
      return meta.__json_serialise(v, pretty)
    else if v.__table_type == 'object' then
      return serialise_object_table_raw(v, pretty)
    else if v.__table_type == 'array' then
      return serialise_array_table_raw(v, pretty)
    else if v[1] ~= nil then
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
  else if pretty then
    return "{\n  " table.concat(fields, ",\n  ") .. "\n}"
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
  else if pretty then
    return "[\n  " table.concat(items, ",\n  ") .. "\n]"
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

local standard_registration
do
  local standard_registration_defaults = {
    type = 'standard',
    properties = {},
    default_extra_info = {},
    extra_info_modifiers = {},
  }
  local standard_registration_meta = {
    __index = standard_registration_defaults,
  }
  function standard_registration(obj)
    return setmetatable(obj, standard_registration_meta)
  end
end
