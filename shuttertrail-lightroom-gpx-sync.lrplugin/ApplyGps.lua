local LrTasks = import "LrTasks"

local M = {}

function M.apply(catalog, results, replaceExisting, progress)
    local applied, preserved, failed, pending = 0, 0, {}, {}
    for _, item in ipairs(results) do
        if item.match then
            if item.record.existingGps and not replaceExisting then
                preserved = preserved + 1
            else
                pending[#pending + 1] = item
            end
        end
    end

    local batchSize = 200
    for first = 1, #pending, batchSize do
        local last = math.min(#pending, first + batchSize - 1)
        catalog:withWriteAccessDo("shuttertrail-lightroom-gpx-sync", function()
            for index = first, last do
                local item = pending[index]
                local ok, err = pcall(function()
                    item.record.photo:setRawMetadata("gps", {
                        latitude = item.match.latitude,
                        longitude = item.match.longitude,
                    })
                    if item.match.altitude then
                        item.record.photo:setRawMetadata("gpsAltitude", item.match.altitude)
                    elseif replaceExisting then
                        item.record.photo:setRawMetadata("gpsAltitude", nil)
                    end
                end)
                if ok then applied = applied + 1
                else failed[#failed + 1] = item.record.fileName .. ": " .. tostring(err) end
            end
        end, { timeout = 60 })
        if progress then
            progress:setCaption(string.format("Writing GPS metadata: %d of %d", last, #pending))
            progress:setPortionComplete(95 + 5 * last / math.max(1, #pending), 100)
        end
        LrTasks.yield()
    end
    return applied, preserved, failed
end

return M
