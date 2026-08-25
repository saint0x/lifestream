obs = obslua

local settings_cache = {
  api_base_url = "http://127.0.0.1:4127",
  broadcast_id = "broadcast_prime_launch",
  user_id = "user_creator_owner",
  role = "creator_owner",
  token = ""
}

function script_description()
  return "Vanta OBS Companion: opens the lightweight Vanta browser dock for sponsor cues, stream health, proof markers, replay markers, and live/archive sync."
end

function script_properties()
  local props = obs.obs_properties_create()
  obs.obs_properties_add_text(props, "api_base_url", "Vanta API base URL", obs.OBS_TEXT_DEFAULT)
  obs.obs_properties_add_text(props, "broadcast_id", "Broadcast ID", obs.OBS_TEXT_DEFAULT)
  obs.obs_properties_add_text(props, "user_id", "Vanta user ID", obs.OBS_TEXT_DEFAULT)
  obs.obs_properties_add_text(props, "role", "Vanta role", obs.OBS_TEXT_DEFAULT)
  obs.obs_properties_add_text(props, "token", "Bearer token", obs.OBS_TEXT_PASSWORD)
  obs.obs_properties_add_button(props, "open_dock", "Open Vanta Dock URL", open_vanta_dock_url)
  return props
end

function script_defaults(settings)
  obs.obs_data_set_default_string(settings, "api_base_url", settings_cache.api_base_url)
  obs.obs_data_set_default_string(settings, "broadcast_id", settings_cache.broadcast_id)
  obs.obs_data_set_default_string(settings, "user_id", settings_cache.user_id)
  obs.obs_data_set_default_string(settings, "role", settings_cache.role)
end

function script_update(settings)
  settings_cache.api_base_url = obs.obs_data_get_string(settings, "api_base_url")
  settings_cache.broadcast_id = obs.obs_data_get_string(settings, "broadcast_id")
  settings_cache.user_id = obs.obs_data_get_string(settings, "user_id")
  settings_cache.role = obs.obs_data_get_string(settings, "role")
  settings_cache.token = obs.obs_data_get_string(settings, "token")
end

function open_vanta_dock_url()
  local url = dock_url()
  obs.script_log(obs.LOG_INFO, "Add this URL as an OBS Custom Browser Dock: " .. url)
  return true
end

function dock_url()
  local script_path = script_path_or_empty()
  local dock_path = script_path .. "dock/index.html"
  return "file://" .. dock_path
    .. "?apiBaseUrl=" .. encode(settings_cache.api_base_url)
    .. "&broadcastId=" .. encode(settings_cache.broadcast_id)
    .. "&userId=" .. encode(settings_cache.user_id)
    .. "&role=" .. encode(settings_cache.role)
    .. "&token=" .. encode(settings_cache.token)
end

function script_path_or_empty()
  if script_path ~= nil then
    return script_path()
  end
  return ""
end

function encode(value)
  value = tostring(value or "")
  value = string.gsub(value, "\n", "")
  value = string.gsub(value, "([^%w%-_%.~])", function(c)
    return string.format("%%%02X", string.byte(c))
  end)
  return value
end
