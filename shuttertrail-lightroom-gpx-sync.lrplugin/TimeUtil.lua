local M = {}

-- Returns days since 1970-01-01 without consulting the computer timezone.
local function daysFromCivil(year, month, day)
    if month <= 2 then year = year - 1 end
    local era = math.floor(year / 400)
    local yoe = year - era * 400
    local mp = month + (month > 2 and -3 or 9)
    local doy = math.floor((153 * mp + 2) / 5) + day - 1
    local doe = yoe * 365 + math.floor(yoe / 4) - math.floor(yoe / 100) + doy
    return era * 146097 + doe - 719468
end

local function epochFromParts(y, m, d, hh, mm, ss)
    return daysFromCivil(y, m, d) * 86400 + hh * 3600 + mm * 60 + ss
end

function M.parseOffset(value)
    if not value or value == "" then return nil, "missing offset" end
    if value == "Z" or value == "z" then return 0 end
    local sign, hh, mm = tostring(value):match("^([+-])(%d%d):?(%d%d)$")
    if not sign then return nil, "offset must look like -07:00 or +05:30" end
    hh, mm = tonumber(hh), tonumber(mm)
    if hh > 14 or mm > 59 or (hh == 14 and mm ~= 0) then
        return nil, "offset is outside the valid range"
    end
    local seconds = hh * 3600 + mm * 60
    if sign == "-" then seconds = -seconds end
    return seconds
end

function M.normalizeOffset(value)
    local seconds, err = M.parseOffset(value)
    if not seconds then return nil, err end
    local sign = seconds < 0 and "-" or "+"
    seconds = math.abs(seconds)
    return string.format("%s%02d:%02d", sign, math.floor(seconds / 3600), math.floor((seconds % 3600) / 60))
end

function M.parsePhotoTime(value, subsec, offset)
    if not value then return nil, "missing DateTimeOriginal" end
    local y, mo, d, h, mi, s = tostring(value):match("^(%d%d%d%d):(%d%d):(%d%d)[ T](%d%d):(%d%d):(%d%d)")
    if not y then
        y, mo, d, h, mi, s = tostring(value):match("^(%d%d%d%d)%-(%d%d)%-(%d%d)[ T](%d%d):(%d%d):(%d%d)")
    end
    if not y then return nil, "unsupported capture timestamp: " .. tostring(value) end
    local offsetSeconds, err = M.parseOffset(offset)
    if offsetSeconds == nil then return nil, err end
    local digits = tostring(subsec or "0"):match("^(%d+)") or "0"
    local fraction = tonumber("0." .. digits) or 0
    local naive = epochFromParts(tonumber(y), tonumber(mo), tonumber(d), tonumber(h), tonumber(mi), tonumber(s)) + fraction
    return naive - offsetSeconds
end

function M.parseIso8601(value)
    if not value then return nil, "missing GPX time" end
    local y, mo, d, h, mi, s, frac, zone = tostring(value):match(
        "^(%d%d%d%d)%-(%d%d)%-(%d%d)T(%d%d):(%d%d):(%d%d)(%.?%d*)([Zz%+%-].*)$"
    )
    if not y then return nil, "unsupported GPX timestamp: " .. tostring(value) end
    local offsetSeconds, err = M.parseOffset(zone)
    if offsetSeconds == nil then return nil, err end
    local fraction = tonumber(frac ~= "" and frac or "0") or 0
    return epochFromParts(tonumber(y), tonumber(mo), tonumber(d), tonumber(h), tonumber(mi), tonumber(s)) + fraction - offsetSeconds
end

return M
