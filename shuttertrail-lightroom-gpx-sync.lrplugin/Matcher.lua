local M = {}

local function sameLocation(a, b)
    return math.abs(a.latitude - b.latitude) < 0.0000001
        and math.abs(a.longitude - b.longitude) < 0.0000001
end

function M.closest(points, epoch, maximumSeconds)
    if #points == 0 then return nil, "no GPX points" end

    local lo, hi = 1, #points
    while lo <= hi do
        local mid = math.floor((lo + hi) / 2)
        if points[mid].epoch < epoch then lo = mid + 1 else hi = mid - 1 end
    end

    local before = hi >= 1 and points[hi] or nil
    local after = lo <= #points and points[lo] or nil
    local selected, selectedIndex

    if before and after then
        local db, da = math.abs(epoch - before.epoch), math.abs(after.epoch - epoch)
        if db <= da then
            selected, selectedIndex = before, hi -- earlier point wins exact ties
        else
            selected, selectedIndex = after, lo
        end
    else
        selected, selectedIndex = before or after, before and hi or lo
    end

    local difference = math.abs(epoch - selected.epoch)
    if difference > maximumSeconds then
        return nil, "closest GPX point is " .. math.floor(difference / 60) .. " minutes away", difference
    end

    -- Reject any conflicting location in the complete equal-timestamp range.
    local first, last = selectedIndex, selectedIndex
    while first > 1 and points[first - 1].epoch == selected.epoch do first = first - 1 end
    while last < #points and points[last + 1].epoch == selected.epoch do last = last + 1 end
    for index = first, last do
        if not sameLocation(points[index], selected) then
            return nil, "conflicting GPX locations share the same timestamp", difference
        end
    end

    return selected, nil, difference
end

return M
