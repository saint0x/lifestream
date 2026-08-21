wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"

request = function()
  return wrk.format(
    nil,
    "/api/v1/playback/live/lv-deepsaint-live/session",
    nil,
    "{\"deviceId\":\"wrk-load-device\",\"deviceName\":\"wrk load probe\",\"playerVersion\":\"1.0.0\",\"capabilities\":{\"lowLatency\":true,\"hls\":true}}"
  )
end
