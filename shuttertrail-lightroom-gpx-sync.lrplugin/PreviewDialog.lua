local LrBinding = import "LrBinding"
local LrDialogs = import "LrDialogs"
local LrFunctionContext = import "LrFunctionContext"
local LrView = import "LrView"

local M = {}

function M.show(results, selectionSummary, offsetSummary)
    local matched, matchedWithExistingLocation = 0, 0
    local withEmbeddedOffset, withoutEmbeddedOffset = 0, 0
    local usingManualOffset, skippedWithoutManualOffset = 0, 0
    for _, item in ipairs(results) do
        if item.match then
            matched = matched + 1
            if item.record.existingGps then
                matchedWithExistingLocation = matchedWithExistingLocation + 1
            end
        end
        if item.offsetSource and item.offsetSource:match("^EXIF") then
            withEmbeddedOffset = withEmbeddedOffset + 1
        else
            withoutEmbeddedOffset = withoutEmbeddedOffset + 1
        end
        if item.offsetSource and item.offsetSource:match("^user") then
            usingManualOffset = usingManualOffset + 1
        end
        if item.reason == "no UTC offset supplied" then
            skippedWithoutManualOffset = skippedWithoutManualOffset + 1
        end
    end

    selectionSummary = selectionSummary or {
        totalSelected = #results,
        photoCount = #results,
        videoCount = 0,
    }

    local matchedWithoutExistingLocation = matched - matchedWithExistingLocation
    local detectedOffsetLines = {}
    for index, entry in ipairs(offsetSummary or {}) do
        local suffix = index == 1 and " (most detected)" or ""
        detectedOffsetLines[#detectedOffsetLines + 1] = string.format(
            "%s: %d photo%s%s",
            entry.offset,
            entry.count,
            entry.count == 1 and "" or "s",
            suffix)
    end
    if #detectedOffsetLines == 0 then
        detectedOffsetLines[1] = "No embedded offsets detected"
    end

    local approved, replaceExisting = false, false
    LrFunctionContext.callWithContext("shuttertrail-lightroom-gpx-sync-preview-dialog", function(context)
        local props = LrBinding.makePropertyTable(context)
        props.existingLocationAction = "preserve"

        local f = LrView.osFactory()
        local function countRow(label, count, bold)
            return f:row {
                spacing = f:control_spacing(),
                f:static_text { title = label, width = 380 },
                f:static_text {
                    title = tostring(count),
                    width = 60,
                    alignment = "right",
                    font = bold and "<system/bold>" or nil,
                },
            }
        end

        local actionChoice = f:static_text {
            title = "No matched photos have an existing location.",
        }
        if matchedWithExistingLocation > 0 then
            actionChoice = f:column {
                bind_to_object = props,
                spacing = f:control_spacing(),
                f:radio_button {
                    title = "Preserve existing locations",
                    value = LrView.bind("existingLocationAction"),
                    checked_value = "preserve",
                },
                f:radio_button {
                    title = "Replace existing locations",
                    value = LrView.bind("existingLocationAction"),
                    checked_value = "replace",
                },
            }
        end

        local contents = f:column {
            spacing = f:control_spacing(),
            width = 470,

            f:static_text { title = "Selected files", font = "<system/bold>" },
            countRow("Total files selected", selectionSummary.totalSelected),
            countRow("Photos", selectionSummary.photoCount),
            countRow("Videos (ignored)", selectionSummary.videoCount),

            f:static_text { title = " " },
            f:static_text { title = "Photo time offsets", font = "<system/bold>" },
            countRow("With an offset stored in the photo", withEmbeddedOffset),
            countRow("Without an offset stored in the photo", withoutEmbeddedOffset),
            f:static_text { title = "Detected offsets and counts:" },
            f:static_text { title = table.concat(detectedOffsetLines, "\n"), width = 440 },
            countRow("Using an offset confirmed in the prompt", usingManualOffset),
            countRow("Skipped because no offset was provided", skippedWithoutManualOffset),

            f:static_text { title = " " },
            f:static_text { title = "GPX matching results", font = "<system/bold>" },
            countRow("Photos with a matching GPX location", matched, true),
            countRow("Matched photos that already have a location", matchedWithExistingLocation),
            countRow("Matched photos that do not have a location yet", matchedWithoutExistingLocation, true),

            f:static_text { title = " " },
            f:static_text { title = "Existing locations", font = "<system/bold>" },
            actionChoice,
            countRow("Preserve existing — photos updated", matchedWithoutExistingLocation, true),
            countRow("Replace existing — photos updated", matched, true),
            f:static_text {
                title = "Only matches no more than one hour from a GPX point will be applied.",
            },
        }

        local answer = LrDialogs.presentModalDialog {
            title = "shuttertrail-lightroom-gpx-sync — Preview",
            contents = contents,
            actionVerb = "Apply",
            cancelVerb = "Cancel",
            resizable = true,
        }
        approved = answer == "ok"
        replaceExisting = approved and props.existingLocationAction == "replace"
    end)
    return approved, replaceExisting
end

return M
