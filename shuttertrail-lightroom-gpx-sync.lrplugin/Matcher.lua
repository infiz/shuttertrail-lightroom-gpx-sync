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
    local selected

    if before and after then
        local db, da = math.abs(epoch - before.epoch), math.abs(after.epoch - epoch)
        selected = db <= da and before or after -- earlier point wins exact ties
    else
        selected = before or after
    end

    local difference = math.abs(epoch - selected.epoch)
    if difference > maximumSeconds then
        return nil, "closest GPX point is " .. math.floor(difference / 60) .. " minutes away", difference
    end

    -- Equal timestamps at different locations are ambiguous.
    local index
    for i = math.max(1, lo - 2), math.min(#points, lo + 2) do
        if points[i] == selected then index = i break end
    end
    if index then
        local left, right = points[index - 1], points[index + 1]
        if (left and left.epoch == selected.epoch and not sameLocation(left, selected))
            or (right and right.epoch == selected.epoch and not sameLocation(right, selected)) then
            return nil, "conflicting GPX locations share the same timestamp", difference
        end
    end

    return selected, nil, difference
end

return M
