wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"
wrk.headers["Authorization"] = "Bearer vanta-local-dev-token"

local counter = 0

request = function()
  counter = counter + 1
  return wrk.format(
    nil,
    "/api/v1/live/streams/lv-deepsaint-live/chat/messages",
    nil,
    string.format("{\"body\":\"wrk live chat %d\"}", counter)
  )
end
