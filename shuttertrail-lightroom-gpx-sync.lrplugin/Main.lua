local LrApplication = import "LrApplication"
local LrDialogs = import "LrDialogs"
local LrProgressScope = import "LrProgressScope"
local LrTasks = import "LrTasks"

local ApplyGps = require "ApplyGps"
local GpxParser = require "GpxParser"
local Logger = require "Logger"
local Matcher = require "Matcher"
local MetadataReader = require "MetadataReader"
local OffsetDialog = require "OffsetDialog"
local PreviewDialog = require "PreviewDialog"
local TimeUtil = require "TimeUtil"

local MAXIMUM_DIFFERENCE_SECONDS = 60 * 60

local function withoutVideos(items)
    local photos = {}
    for _, item in ipairs(items or {}) do
        if not item:getRawMetadata("isVideo") then
            photos[#photos + 1] = item
        end
    end
    return photos
end

local function chooseGpxFiles()
    return LrDialogs.runOpenPanel {
        title = "Choose one or more GPX files",
        prompt = "Use GPX Files",
        canChooseFiles = true,
        canChooseDirectories = false,
        canCreateDirectories = false,
        allowsMultipleSelection = true,
        fileTypes = { "gpx", "xml" },
    }
end

local function validEmbeddedOffset(value)
    if not value then return nil end
    return TimeUtil.normalizeOffset(value)
end

local function summarizeDetectedOffsets(records)
    local counts = {}
    for _, record in ipairs(records) do
        local offset = validEmbeddedOffset(record.embeddedOffset)
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
    return summary, summary[1] and summary[1].offset or nil
end

local function calculateResults(records, points, progress)
    local results = {}
    local globalOffset, cameraOffsets, skipRemaining = nil, {}, false
    local offsetSummary, mostDetectedOffset = summarizeDetectedOffsets(records)

    for index, record in ipairs(records) do
        if progress and progress:isCanceled() then return nil, true end
        if progress and (index == 1 or index % 25 == 0 or index == #records) then
            progress:setCaption(string.format("Matching locations: %d of %d", index, #records))
            progress:setPortionComplete(90 + 5 * index / math.max(1, #records), 100)
            LrTasks.yield()
        end
        local offset = validEmbeddedOffset(record.embeddedOffset)
        local offsetSource = offset and ("EXIF " .. offset) or nil

        if not offset then
            if mostDetectedOffset then
                offset = mostDetectedOffset
                offsetSource = "most detected " .. mostDetectedOffset
            elseif globalOffset then
                offset, offsetSource = globalOffset, "user (all) " .. globalOffset
            elseif cameraOffsets[record.cameraKey] then
                offset = cameraOffsets[record.cameraKey]
                offsetSource = "user (camera) " .. offset
            elseif not skipRemaining then
                local selected, scope = OffsetDialog.ask(record, #records - index + 1)
                if selected then
                    offset = selected
                    offsetSource = "user " .. selected
                    if scope == "all" then globalOffset = selected end
                    if scope == "camera" then cameraOffsets[record.cameraKey] = selected end
                else
                    skipRemaining = true
                end
            end
        end

        if not offset then
            results[#results + 1] = { record = record, reason = "no UTC offset supplied" }
        else
            local photoEpoch, timeErr = TimeUtil.parsePhotoTime(record.captureTime, record.subsec, offset)
            if not photoEpoch then
                results[#results + 1] = { record = record, reason = timeErr, offsetSource = offsetSource }
            else
                local point, matchErr, difference = Matcher.closest(points, photoEpoch, MAXIMUM_DIFFERENCE_SECONDS)
                results[#results + 1] = {
                    record = record,
                    photoEpoch = photoEpoch,
                    offsetSource = offsetSource,
                    match = point,
                    reason = matchErr,
                    difference = difference,
                }
            end
        end
    end
    return results, false, offsetSummary
end

LrTasks.startAsyncTask(function()
    local catalog = LrApplication.activeCatalog()

    local gpxPaths = chooseGpxFiles()
    if not gpxPaths or #gpxPaths == 0 then return end

    local selectedItems = catalog:getTargetPhotos() or {}
    local photos = withoutVideos(selectedItems)
    local selectionSummary = {
        totalSelected = #selectedItems,
        photoCount = #photos,
        videoCount = #selectedItems - #photos,
    }
    if not photos or #photos == 0 then
        LrDialogs.message("shuttertrail-lightroom-gpx-sync", "No still photos are selected. Video files are currently ignored.", "info")
        return
    end

    local progress = LrProgressScope {
        title = "shuttertrail-lightroom-gpx-sync",
        caption = "Reading GPX tracks…",
    }
    progress:setCancelable(true)
    progress:setPortionComplete(0, 100)
    LrTasks.yield()

    local points, parseErrors = GpxParser.parseFiles(gpxPaths)
    if #points == 0 then
        progress:done()
        LrDialogs.message("No usable GPX points",
            "The selected files contained no timestamped track, route, or waypoint records.\n\n"
                .. table.concat(parseErrors, "\n"), "critical")
        return
    end

    progress:setCaption(string.format("Reading metadata from %d selected items…", #photos))
    progress:setPortionComplete(0, 100)
    LrTasks.yield()
    local records, metadataErr, exifVersion = MetadataReader.read(photos, progress)
    if not records then
        progress:done()
        if metadataErr == "canceled" then return end
        LrDialogs.message("Metadata reader unavailable", metadataErr, "critical")
        return
    end
    if #records == 0 then
        progress:done()
        LrDialogs.message("No readable photos", "ExifTool could not read any of the selected originals.", "critical")
        return
    end
    if progress:isCanceled() then
        progress:done()
        return
    end
    progress:setPortionComplete(90, 100)

    Logger.info(string.format("ExifTool %s read %d photos; parsed %d GPX points",
        tostring(exifVersion), #records, #points))

    local results, canceled, offsetSummary = calculateResults(records, points, progress)
    if canceled then
        progress:done()
        return
    end
    progress:setCaption("Waiting for preview confirmation…")
    progress:setPortionComplete(95, 100)
    local approved, replaceExisting = PreviewDialog.show(
        results, #gpxPaths, #points, selectionSummary, offsetSummary)
    if not approved then
        progress:done()
        return
    end

    progress:setCancelable(false)
    progress:setCaption("Writing GPS metadata to the Lightroom catalog…")
    local applied, preserved, failures = ApplyGps.apply(catalog, results, replaceExisting, progress)
    progress:setPortionComplete(100, 100)
    progress:done()
    local message = string.format("Applied GPS to %d photos.\nPreserved existing GPS on %d photos.", applied, preserved)
    if #failures > 0 then
        message = message .. "\n\nFailures:\n" .. table.concat(failures, "\n")
    end
    if #parseErrors > 0 then
        message = message .. "\n\nGPX warnings: " .. #parseErrors
    end
    LrDialogs.message("shuttertrail-lightroom-gpx-sync complete", message, #failures > 0 and "warning" or "info")
end)
