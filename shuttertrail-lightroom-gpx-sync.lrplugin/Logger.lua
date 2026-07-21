local LrLogger = import "LrLogger"

local logger = LrLogger("shuttertrail-lightroom-gpx-sync")
logger:enable("logfile")

local M = {}

function M.info(message)
    logger:info(tostring(message))
end

return M
