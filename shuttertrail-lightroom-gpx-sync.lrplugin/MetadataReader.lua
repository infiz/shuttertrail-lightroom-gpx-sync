local LrFileUtils = import "LrFileUtils"
local LrPathUtils = import "LrPathUtils"
local LrTasks = import "LrTasks"

local M = {}

local function quote(value)
    value = tostring(value)
    if WIN_ENV then
        return '"' .. value:gsub('"', '\\"') .. '"'
    end
    return "'" .. value:gsub("'", "'\\''") .. "'"
end

local function tempPath(extension)
    local base = LrPathUtils.getStandardFilePath("temp")
    local token = tostring(os.time()) .. "-" .. tostring(math.random(100000, 999999))
    return LrPathUtils.child(base, "shuttertrail-lightroom-gpx-sync-" .. token .. extension)
end

local function readFile(path)
    local f = io.open(path, "rb")
    if not f then return nil end
    local data = f:read("*a")
    f:close()
    return data
end

local function writeFile(path, data)
    local f, err = io.open(path, "wb")
    if not f then return nil, err end
    f:write(data)
    f:close()
    return true
end

local function executableCandidates()
    local candidates = {}
    local bin = LrPathUtils.child(_PLUGIN.path, "bin")
    if WIN_ENV then
        local windowsBin = LrPathUtils.child(bin, "windows")
        candidates[#candidates + 1] = LrPathUtils.child(windowsBin, "exiftool.exe")
        candidates[#candidates + 1] = "exiftool.exe"
        candidates[#candidates + 1] = "exiftool"
    else
        local macBin = LrPathUtils.child(bin, "macos")
        candidates[#candidates + 1] = LrPathUtils.child(macBin, "exiftool")
        candidates[#candidates + 1] = "/usr/local/bin/exiftool"
        candidates[#candidates + 1] = "/opt/homebrew/bin/exiftool"
        candidates[#candidates + 1] = "exiftool"
    end
    return candidates
end

local function discoverExifTool()
    local candidates = executableCandidates()
    local bundled = candidates[1]
    if LrFileUtils.exists(bundled) then
        return bundled, "bundled"
    end

    for index, candidate in ipairs(candidates) do
        if index > 1 then
        local output = tempPath(".version.txt")
        local command = quote(candidate) .. " -ver > " .. quote(output) .. " 2>&1"
        local status = LrTasks.execute(command)
        local version = readFile(output)
        if LrFileUtils.exists(output) then LrFileUtils.delete(output) end
        if status == 0 and version and version:match("%d+%.%d+") then
            return candidate, version:match("%d+%.%d+")
        end
        end
    end
    return nil, bundled
end

local function standardized(path)
    local value = LrPathUtils.standardizePath(path or "")
    return WIN_ENV and value:lower() or value
end

local function valueOrNil(value)
    if value == nil or value == "" or value == "-" then return nil end
    return value
end

local function splitTabs(line)
    local values = {}
    for value in (line .. "\t"):gmatch("(.-)\t") do
        values[#values + 1] = valueOrNil(value)
    end
    return values
end

local function offsetFromTimestamp(value)
    if not value then return nil end
    return tostring(value):match("([+-]%d%d:?%d%d)$")
        or tostring(value):match("([Zz])$")
end

local function subsecFromTimestamp(value)
    if not value then return nil end
    return tostring(value):match(":%d%d%.(%d+)")
end

local function usableTimestamp(value)
    if not value or value == "" or value == "-" then return nil end
    if tostring(value):match("^0000[:%-]") then return nil end
    return value
end

local function readBatch(photos, executable)
    local argPath, jsonPath, errorPath = tempPath(".args.txt"), tempPath(".tsv"), tempPath(".error.txt")
    local lines = {
        "-T", "-n", "-charset", "filename=UTF8",
        "-Directory", "-FileName",
        "-EXIF:DateTimeOriginal", "-EXIF:SubSecTimeOriginal", "-EXIF:OffsetTimeOriginal",
        "-Keys:CreationDate", "-UserData:DateTimeOriginal", "-QuickTime:CreateDate",
        "-Make", "-Model", "-SerialNumber", "-InternalSerialNumber", "-BodySerialNumber",
    }
    local photoByPath = {}
    for _, photo in ipairs(photos) do
        local path = photo:getRawMetadata("path")
        if path and path ~= "" then
            lines[#lines + 1] = path
            photoByPath[standardized(path)] = photo
        end
    end

    local ok, writeErr = writeFile(argPath, table.concat(lines, "\n") .. "\n")
    if not ok then return nil, writeErr end

    local command
    local wrapperPath
    if WIN_ENV then
        wrapperPath = tempPath(".cmd")
        local wrapper = "@echo off\r\n"
            .. quote(executable) .. " -@ " .. quote(argPath)
            .. " 1> " .. quote(jsonPath) .. " 2> " .. quote(errorPath) .. "\r\n"
            .. "exit /b %ERRORLEVEL%\r\n"
        local wrapperOk, wrapperErr = writeFile(wrapperPath, wrapper)
        if not wrapperOk then return nil, "Could not create ExifTool launcher: " .. tostring(wrapperErr) end
        command = "cmd.exe /d /s /c " .. quote(wrapperPath)
    else
        command = quote(executable) .. " -@ " .. quote(argPath)
            .. " > " .. quote(jsonPath) .. " 2> " .. quote(errorPath)
    end

    local status = LrTasks.execute(command)
    local json, stderr = readFile(jsonPath), readFile(errorPath)
    LrFileUtils.delete(argPath)
    if LrFileUtils.exists(jsonPath) then LrFileUtils.delete(jsonPath) end
    if LrFileUtils.exists(errorPath) then LrFileUtils.delete(errorPath) end
    if wrapperPath and LrFileUtils.exists(wrapperPath) then LrFileUtils.delete(wrapperPath) end

    if status ~= 0 or not json then
        return nil, "ExifTool failed.\n\nExit status: " .. tostring(status)
            .. "\nExecutable: " .. tostring(executable)
            .. "\nDetails: " .. tostring(stderr or "No diagnostic output was produced.")
    end

    local result = {}
    for line in json:gmatch("[^\r\n]+") do
        local values = splitTabs(line)
        local sourceFile = values[1] and values[2] and LrPathUtils.child(values[1], values[2]) or nil
        local photo = sourceFile and photoByPath[standardized(sourceFile)] or nil
        if photo then
            local isVideo = photo:getRawMetadata("isVideo") == true
            local videoMetadataTime = usableTimestamp(values[6]) or usableTimestamp(values[7]) or usableTimestamp(values[8])
            local captureTime = isVideo
                and (videoMetadataTime or photo:getRawMetadata("dateTimeOriginalISO8601"))
                or values[3]
            local subsec = isVideo and subsecFromTimestamp(captureTime) or values[4]
            local embeddedOffset = values[5] or (videoMetadataTime and offsetFromTimestamp(videoMetadataTime))
            local serial = values[11] or values[13] or values[12]
            result[#result + 1] = {
                photo = photo,
                path = sourceFile,
                fileName = LrPathUtils.leafName(sourceFile),
                captureTime = captureTime,
                subsec = subsec,
                embeddedOffset = embeddedOffset,
                make = values[9],
                model = values[10],
                serial = serial,
                cameraKey = table.concat({ values[9] or "", values[10] or "", serial or "" }, "|"),
                existingGps = photo:getRawMetadata("gps"),
                isVideo = isVideo,
            }
        end
    end

    return result
end

function M.read(photos, progress)
    local executable, version = discoverExifTool()
    if not executable then
        return nil, "ExifTool was not found. Expected the bundled executable at:\n"
            .. tostring(version) .. "\n\nReload the plug-in after confirming that file exists."
    end

    local result = {}
    local batchSize = 50
    local total = #photos
    for first = 1, total, batchSize do
        if progress and progress:isCanceled() then return nil, "canceled" end
        local last = math.min(total, first + batchSize - 1)
        if progress then
            progress:setCaption(string.format("Reading metadata: %d / %d", first - 1, total))
            progress:setPortionComplete(90 * (first - 1) / math.max(1, total), 100)
            LrTasks.yield()
        end

        local batch = {}
        for index = first, last do batch[#batch + 1] = photos[index] end
        local records, err = readBatch(batch, executable)
        if not records then return nil, err end
        for _, record in ipairs(records) do result[#result + 1] = record end

        if progress then
            progress:setCaption(string.format("Reading metadata: %d / %d", last, total))
            progress:setPortionComplete(90 * last / math.max(1, total), 100)
            LrTasks.yield()
        end
    end

    return result, nil, version
end

return M
