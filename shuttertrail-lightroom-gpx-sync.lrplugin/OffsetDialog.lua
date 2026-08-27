local LrBinding = import "LrBinding"
local LrDialogs = import "LrDialogs"
local LrFunctionContext = import "LrFunctionContext"
local LrView = import "LrView"

local TimeUtil = require "TimeUtil"

local M = {}

local function cameraLabel(record)
    local parts = {}
    if record.make and record.make ~= "" then parts[#parts + 1] = record.make end
    if record.model and record.model ~= "" then parts[#parts + 1] = record.model end
    return #parts > 0 and table.concat(parts, " ") or "Unknown camera"
end

function M.ask(record, remainingCount, suggestedOffset, progress)
    local selectedOffset, selectedScope, cancelled
    progress = progress or { current = 1, total = math.max(1, remainingCount), suggestedCount = 0 }

    LrFunctionContext.callWithContext("shuttertrail-lightroom-gpx-sync-offset-dialog", function(context)
        local props = LrBinding.makePropertyTable(context)
        props.offset = suggestedOffset or "+00:00"
        props.scope = progress.total > progress.current and "all" or "one"

        local f = LrView.osFactory()
        local labelWidth = 105
        local facts = f:group_box {
            title = "Photo details",
            spacing = f:control_spacing(),
            fill_horizontal = 1,
            f:row {
                f:static_text { title = "PHOTO", width = labelWidth, font = "<system/small/bold>" },
                f:static_text { title = tostring(record.fileName or "Unnamed photo"), width = 400 },
            },
            f:row {
                f:static_text { title = "CAMERA", width = labelWidth, font = "<system/small/bold>" },
                f:static_text { title = cameraLabel(record), width = 400 },
            },
            f:row {
                f:static_text { title = "CAPTURE TIME", width = labelWidth, font = "<system/small/bold>" },
                f:static_text { title = tostring(record.captureTime or "Unavailable"), width = 400 },
            },
        }

        local suggestion
        if suggestedOffset then
            suggestion = string.format(
                "Suggested %s — the most-used offset detected in the other photos (%d %s).",
                suggestedOffset,
                progress.suggestedCount or 0,
                (progress.suggestedCount or 0) == 1 and "photo" or "photos")
        else
            suggestion = "No embedded offsets were detected, so the prompt starts at +00:00."
        end

        local contents = f:column {
            bind_to_object = props,
            spacing = f:dialog_spacing(),
            width = 540,

            f:static_text { title = "SHUTTERTRAIL  ·  MISSING UTC OFFSET", font = "<system/small/bold>" },
            f:static_text {
                title = "No absolute time was found in this photo.",
                font = "<system/bold>",
                width = 520,
            },
            f:static_text {
                title = string.format("Photo %d of %d requiring an offset",
                    progress.current or 1, progress.total or 1),
                font = "<system/small>",
            },

            facts,

            f:group_box {
                title = "Offset decision",
                spacing = f:control_spacing(),
                fill_horizontal = 1,
                f:row {
                    spacing = f:control_spacing(),
                    f:static_text { title = "UTC offset", width = labelWidth, font = "<system/bold>" },
                    f:edit_field { value = LrView.bind("offset"), width_in_chars = 9 },
                    f:static_text { title = "Example: -07:00", font = "<system/small>" },
                },
                f:static_text {
                    title = suggestion,
                    width = 490,
                    height_in_lines = -1,
                    font = "<system/small>",
                },
                f:row {
                    spacing = f:control_spacing(),
                    f:static_text { title = "Use for", width = labelWidth, font = "<system/bold>" },
                    f:popup_menu {
                        value = LrView.bind("scope"),
                        items = {
                            { title = "All remaining photos without an embedded offset", value = "all" },
                            { title = "Remaining photos from this camera", value = "camera" },
                            { title = "This photo only", value = "one" },
                        },
                        width_in_chars = 43,
                    },
                },
            },

            f:static_text {
                title = "The offset converts the camera's capture time to UTC before it is matched to the GPX track.",
                width = 520,
                height_in_lines = -1,
                font = "<system/small>",
            },
        }

        while true do
            local result = LrDialogs.presentModalDialog {
                title = "ShutterTrail GeoTagger — Missing UTC Offset",
                contents = contents,
                actionVerb = "Use Offset",
                cancelVerb = "Skip Photos Without Offset",
                resizable = true,
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
