local LrBinding = import "LrBinding"
local LrDialogs = import "LrDialogs"
local LrFunctionContext = import "LrFunctionContext"
local LrView = import "LrView"

local M = {}
local MAXIMUM_DETAIL_ROWS = 500

local function plural(count, singular, pluralForm)
    return count == 1 and singular or (pluralForm or singular .. "s")
end

local function formatDifference(seconds)
    if not seconds then return nil end
    if seconds < 60 then return string.format("%.1f sec", seconds) end
    local minutes = math.floor(seconds / 60)
    local remainder = math.floor(seconds % 60)
    return string.format("%d min %02d sec", minutes, remainder)
end

local function detailsTable(f, results)
    local columnWidths = {
        status = 52,
        photo = 145,
        details = 205,
        difference = 78,
        gps = 72,
    }
    local function cell(title, width, options)
        options = options or {}
        return f:static_text {
            title = title,
            width = width,
            alignment = options.alignment or "left",
            truncation = options.truncation,
            font = options.font,
        }
    end
    local function row(values, header)
        local font = header and "<system/small/bold>" or "<system/small>"
        return f:row {
            spacing = f:control_spacing(),
            cell(values.status, columnWidths.status, { font = font }),
            cell(values.photo, columnWidths.photo, { font = font, truncation = "middle" }),
            cell(values.details, columnWidths.details, { font = font, truncation = "tail" }),
            cell(values.difference, columnWidths.difference, { font = font, alignment = "right" }),
            cell(values.gps, columnWidths.gps, { font = font }),
        }
    end

    local bodyItems = { spacing = 3, width = 575 }
    local displayedCount = math.min(#results, MAXIMUM_DETAIL_ROWS)
    for index = 1, displayedCount do
        local item = results[index]
        bodyItems[#bodyItems + 1] = row {
            status = item.match and "MATCH" or "SKIP",
            photo = tostring(item.record.fileName or "Unnamed photo"),
            details = item.match
                and string.format("%.5f, %.5f", item.match.latitude, item.match.longitude)
                or tostring(item.reason or "No GPX match"),
            difference = formatDifference(item.difference) or "—",
            gps = item.record.existingGps and "Existing" or "None",
        }
    end
    if displayedCount == 0 then
        bodyItems[#bodyItems + 1] = f:static_text {
            title = "No readable photos were available for preview.",
            width = 560,
            font = "<system/small>",
        }
    end

    local tableItems = {
        spacing = f:control_spacing(),
        width = 585,
        row({
            status = "STATUS",
            photo = "PHOTO",
            details = "COORDINATES OR REASON",
            difference = "DIFFERENCE",
            gps = "GPS",
        }, true),
        f:separator { fill_horizontal = 1 },
        f:scrolled_view {
            width = 585,
            height = 210,
            horizontal_scroller = false,
            vertical_scroller = true,
            f:column(bodyItems),
        },
    }
    if #results > displayedCount then
        tableItems[#tableItems + 1] = f:static_text {
            title = string.format("Showing the first %d of %d photos.", displayedCount, #results),
            width = 575,
            font = "<system/small>",
        }
    end
    return f:column(tableItems)
end

function M.show(results, selectionSummary, offsetSummary, sourceSummary)
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
    sourceSummary = sourceSummary or {}

    local matchedWithoutExistingLocation = matched - matchedWithExistingLocation
    local notMatched = #results - matched
    local detectedOffsetLines = {}
    for index, entry in ipairs(offsetSummary or {}) do
        local suffix = index == 1 and " (most detected)" or ""
        detectedOffsetLines[#detectedOffsetLines + 1] = string.format(
            "%s  —  %d %s%s",
            entry.offset,
            entry.count,
            plural(entry.count, "photo"),
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
        local function metric(label, count)
            return f:group_box {
                title = label,
                width = 140,
                f:static_text {
                    title = tostring(count),
                    width = 116,
                    alignment = "center",
                    font = "<system/bold>",
                },
            }
        end

        local readyCount = LrView.bind {
            key = "existingLocationAction",
            transform = function(value)
                local count = value == "replace" and matched or matchedWithoutExistingLocation
                return tostring(count) .. " " .. plural(count, "photo") .. " ready to update"
            end,
        }
        local applyExplanation = LrView.bind {
            key = "existingLocationAction",
            transform = function(value)
                local count = value == "replace" and matched or matchedWithoutExistingLocation
                local preserved = value == "preserve" and matchedWithExistingLocation or 0
                local message = string.format("Apply will add GPX locations to %d matched %s.",
                    count, plural(count, "photo"))
                if preserved > 0 then
                    message = message .. string.format(" %d existing %s will remain unchanged.",
                        preserved, plural(preserved, "location"))
                elseif value == "replace" and matchedWithExistingLocation > 0 then
                    message = message .. string.format(" %d existing %s will be replaced.",
                        matchedWithExistingLocation, plural(matchedWithExistingLocation, "location"))
                end
                return message
            end,
        }

        local locationChoice
        if matchedWithExistingLocation > 0 then
            locationChoice = f:group_box {
                title = "Existing locations",
                bind_to_object = props,
                spacing = f:control_spacing(),
                fill_horizontal = 1,
                f:radio_button {
                    title = string.format("Preserve existing locations — update %d %s",
                        matchedWithoutExistingLocation, plural(matchedWithoutExistingLocation, "photo")),
                    value = LrView.bind("existingLocationAction"),
                    checked_value = "preserve",
                },
                f:static_text {
                    title = string.format("Leave %d matched %s with GPS unchanged.",
                        matchedWithExistingLocation, plural(matchedWithExistingLocation, "photo")),
                    font = "<system/small>",
                    width = 570,
                },
                f:radio_button {
                    title = string.format("Replace existing locations — update all %d matched %s",
                        matched, plural(matched, "photo")),
                    value = LrView.bind("existingLocationAction"),
                    checked_value = "replace",
                },
                f:static_text {
                    title = "Existing Lightroom GPS coordinates will be replaced by the GPX match.",
                    font = "<system/small/bold>",
                    width = 570,
                },
            }
        else
            locationChoice = f:group_box {
                title = "Existing locations",
                fill_horizontal = 1,
                f:static_text {
                    title = "No matched photo has an existing location.",
                    font = "<system/bold>",
                    width = 570,
                },
                f:static_text {
                    title = string.format("All %d matches can be applied without replacing GPS metadata.", matched),
                    width = 570,
                },
            }
        end

        local body = f:column {
            bind_to_object = props,
            spacing = f:dialog_spacing(),
            width = 620,

            f:static_text { title = "SHUTTERTRAIL  ·  MATCH PREVIEW", font = "<system/small/bold>" },
            f:static_text { title = readyCount, font = "<system/bold>", width = 600 },
            f:static_text {
                title = "Review the GPX matches and choose how Lightroom should handle photos that already have a location.",
                width = 600,
                height_in_lines = -1,
            },

            f:row {
                spacing = f:control_spacing(),
                metric("TRACK POINTS", sourceSummary.trackPointCount or 0),
                metric("GPX MATCHES", matched),
                metric("EXISTING GPS", matchedWithExistingLocation),
                metric("NOT MATCHED", notMatched),
            },

            locationChoice,

            f:group_box {
                title = "Photo time offsets",
                spacing = f:control_spacing(),
                fill_horizontal = 1,
                f:row {
                    spacing = f:control_spacing(),
                    f:static_text { title = string.format("%d embedded", withEmbeddedOffset), width = 130 },
                    f:static_text { title = string.format("%d confirmed", usingManualOffset), width = 130 },
                    f:static_text { title = string.format("%d skipped", skippedWithoutManualOffset), width = 130 },
                    f:static_text { title = string.format("%d missing", withoutEmbeddedOffset), width = 130 },
                },
                f:static_text {
                    title = table.concat(detectedOffsetLines, "\n"),
                    width = 570,
                    font = "<system/small>",
                },
            },

            f:group_box {
                title = "Match details",
                fill_horizontal = 1,
                detailsTable(f, results),
            },

            f:separator { fill_horizontal = 1 },
            f:static_text { title = applyExplanation, font = "<system/bold>", width = 600, height_in_lines = -1 },
            f:static_text {
                title = string.format(
                    "Matches farther than %d minutes are skipped. Only the Lightroom catalog is updated; original photo files are not edited.%s",
                    math.floor((sourceSummary.maximumDifferenceSeconds or 3600) / 60),
                    selectionSummary.videoCount > 0 and string.format(" %d selected %s ignored.",
                        selectionSummary.videoCount, plural(selectionSummary.videoCount, "video was", "videos were")) or ""),
                font = "<system/small>",
                width = 600,
                height_in_lines = -1,
            },
        }

        local contents = f:scrolled_view {
            width = 650,
            height = 540,
            horizontal_scroller = false,
            vertical_scroller = true,
            body,
        }

        local answer = LrDialogs.presentModalDialog {
            title = "ShutterTrail GeoTagger — Match Preview",
            contents = contents,
            actionVerb = "Apply GPS",
            actionBinding = {
                enabled = {
                    bind_to_object = props,
                    key = "existingLocationAction",
                    transform = function(value)
                        return (value == "replace" and matched or matchedWithoutExistingLocation) > 0
                    end,
                },
            },
            cancelVerb = "Cancel",
            resizable = true,
        }
        approved = answer == "ok"
        replaceExisting = approved and props.existingLocationAction == "replace"
    end)
    return approved, replaceExisting
end

return M
