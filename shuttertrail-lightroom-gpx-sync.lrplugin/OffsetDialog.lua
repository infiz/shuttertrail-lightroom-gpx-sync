local LrBinding = import "LrBinding"
local LrDialogs = import "LrDialogs"
local LrFunctionContext = import "LrFunctionContext"
local LrView = import "LrView"

local TimeUtil = require "TimeUtil"

local M = {}

function M.ask(record, remainingCount)
    local selectedOffset, selectedScope, cancelled
    LrFunctionContext.callWithContext("shuttertrail-lightroom-gpx-sync-offset-dialog", function(context)
        local props = LrBinding.makePropertyTable(context)
        props.offset = "+00:00"
        props.scope = remainingCount > 1 and "all" or "one"

        local f = LrView.osFactory()
        local contents = f:column {
            bind_to_object = props,
            spacing = f:control_spacing(),
            f:static_text { title = "No UTC offset was found in this photo.", font = "<system/bold>" },
            f:static_text { title = "Photo: " .. tostring(record.fileName) },
            f:static_text { title = "Camera: " .. table.concat({ record.make or "Unknown", record.model or "" }, " ") },
            f:static_text { title = "Capture time: " .. tostring(record.captureTime or "Unavailable") },
            f:row {
                f:static_text { title = "UTC offset:", width = 100 },
                f:edit_field { value = LrView.bind("offset"), width_in_chars = 9 },
                f:static_text { title = "Example: -07:00" },
            },
            f:row {
                f:static_text { title = "Use for:", width = 100 },
                f:popup_menu {
                    value = LrView.bind("scope"),
                    items = {
                        { title = "All remaining photos without an embedded offset", value = "all" },
                        { title = "Remaining photos from this camera", value = "camera" },
                        { title = "This photo only", value = "one" },
                    },
                    width_in_chars = 48,
                },
            },
        }

        while true do
            local result = LrDialogs.presentModalDialog {
                title = "shuttertrail-lightroom-gpx-sync — Missing offset",
                contents = contents,
                actionVerb = "Use Offset",
                cancelVerb = "Skip All Photos Without Offset",
            }
            if result ~= "ok" then
                cancelled = true
                return
            end
            local normalized, err = TimeUtil.normalizeOffset(props.offset)
            if normalized then
                selectedOffset, selectedScope = normalized, props.scope
                return
            end
            LrDialogs.message("Invalid UTC offset", err, "critical")
        end
    end)
    if cancelled then return nil, "cancel" end
    return selectedOffset, selectedScope
end

return M
