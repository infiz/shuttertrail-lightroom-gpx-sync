local TimeUtil = require "TimeUtil"

local M = {}

local function readFile(path)
    local f, err = io.open(path, "rb")
    if not f then return nil, err end
    local data = f:read("*a")
    f:close()
    return data
end

local function attr(attributes, name)
    return attributes:match(name .. "%s*=%s*\"([^\"]+)\"")
        or attributes:match(name .. "%s*=%s*'([^']+)'")
end

local function childText(body, name)
    return body:match("<[%w_%-%.]*:?" .. name .. "[^>]*>%s*([^<]-)%s*</[%w_%-%.]*:?" .. name .. ">")
end

local function addPoint(points, source, kind, attributes, body, errors)
    local lat, lon = tonumber(attr(attributes, "lat")), tonumber(attr(attributes, "lon"))
    local timeText = childText(body, "time")
    if not lat or not lon or not timeText then return end
    if lat < -90 or lat > 90 or lon < -180 or lon > 180 then
        errors[#errors + 1] = source .. ": invalid coordinate"
        return
    end
    local epoch, err = TimeUtil.parseIso8601(timeText)
    if not epoch then
        errors[#errors + 1] = source .. ": " .. err
        return
    end
    points[#points + 1] = {
        epoch = epoch,
        latitude = lat,
        longitude = lon,
        altitude = tonumber(childText(body, "ele")),
        source = source,
        kind = kind,
        originalTime = timeText,
    }
end

function M.parseText(xml, source)
    local points, errors = {}, {}
    source = source or "GPX"
    xml = tostring(xml or ""):gsub("^\239\187\191", "")
    for _, kind in ipairs({ "trkpt", "rtept", "wpt" }) do
        for attributes, body in xml:gmatch("<" .. kind .. "([^>]*)>(.-)</" .. kind .. ">") do
            addPoint(points, source, kind, attributes, body, errors)
        end
        for attributes, body in xml:gmatch("<[%w_%-%.]+:" .. kind .. "([^>]*)>(.-)</[%w_%-%.]+:" .. kind .. ">") do
            addPoint(points, source, kind, attributes, body, errors)
        end
    end
    return points, errors
end

function M.parseFiles(paths)
    local points, errors = {}, {}
    for _, path in ipairs(paths) do
        local xml, err = readFile(path)
        if not xml then
            errors[#errors + 1] = path .. ": " .. tostring(err)
        else
            local parsed, parsedErrors = M.parseText(xml, path)
            for _, point in ipairs(parsed) do points[#points + 1] = point end
            for _, parseError in ipairs(parsedErrors) do errors[#errors + 1] = parseError end
        end
    end

    table.sort(points, function(a, b)
        if a.epoch == b.epoch then return a.source < b.source end
        return a.epoch < b.epoch
    end)

    return points, errors
end

return M
