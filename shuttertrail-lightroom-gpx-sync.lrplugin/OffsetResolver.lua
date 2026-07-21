local OffsetDialog = require "OffsetDialog"
local TimeUtil = require "TimeUtil"

local M = {}

local function normalizedEmbeddedOffset(record)
    return record.embeddedOffset and TimeUtil.normalizeOffset(record.embeddedOffset) or nil
end

local function summarize(records)
    local counts = {}
    for _, record in ipairs(records) do
        local offset = normalizedEmbeddedOffset(record)
        if offset then counts[offset] = (counts[offset] or 0) + 1 end
    end

    local summary = {}
    for offset, count in pairs(counts) do
        summary[#summary + 1] = { offset = offset, count = count }
    end
    table.sort(summary, function(left, right)
        if left.count ~= right.count then return left.count > right.count end
        return left.offset < right.offset
    end)
    return summary
end

function M.new(records)
    local summary = summarize(records)
    local state = {
        summary = summary,
        suggestedOffset = summary[1] and summary[1].offset or nil,
        cameraOffsets = {},
        skipRemaining = false,
    }

    function state:resolve(record, remainingCount)
        local embedded = normalizedEmbeddedOffset(record)
        if embedded then return embedded, "EXIF " .. embedded end
        if self.globalOffset then return self.globalOffset, "user (all) " .. self.globalOffset end

        local cameraOffset = self.cameraOffsets[record.cameraKey]
        if cameraOffset then return cameraOffset, "user (camera) " .. cameraOffset end
        if self.skipRemaining then return nil end

        local selected, scope = OffsetDialog.ask(record, remainingCount, self.suggestedOffset)
        if not selected then
            self.skipRemaining = true
            return nil
        end
        if scope == "all" then self.globalOffset = selected end
        if scope == "camera" then self.cameraOffsets[record.cameraKey] = selected end
        return selected, "user " .. selected
    end

    return state
end

return M
