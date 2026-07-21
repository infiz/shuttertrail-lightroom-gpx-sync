# shuttertrail-lightroom-gpx-sync for Lightroom Classic

shuttertrail-lightroom-gpx-sync matches selected Lightroom Classic photos to the nearest timestamped point in one or more GPX files.

## Behavior

- Uses `DateTimeOriginal`, `SubSecTimeOriginal`, and `OffsetTimeOriginal` to calculate an absolute UTC photo time.
- Prompts for a UTC offset when a photo does not contain one.
- Can reuse a supplied offset for all remaining photos or the same camera.
- Searches all selected GPX files for the closest point before or after each photo.
- Accepts a match only when the nearest point is no more than one hour away.
- Writes latitude, longitude, and available altitude into the Lightroom catalog.
- Preserves existing GPS metadata by default, with an explicit preview action to replace it.
- Ignores video files; video timestamp handling is not enabled in this release.
- Shows Lightroom-native progress for metadata reading, matching, and catalog writes.
- Reports processed-item and total counts while reading metadata in ExifTool batches.
- Provides a vertically and horizontally scrollable match preview.
- Allows cancellation before catalog writing begins.
- Never edits original photo files directly.

## Install

1. Keep this entire `shuttertrail-lightroom-gpx-sync.lrplugin` folder in a permanent location.
2. In Lightroom Classic, choose **File > Plug-In Manager**.
3. Click **Add**, select this folder, and confirm that the plug-in is enabled.
4. Select photos and choose **Library > Plug-in Extras > Sync selected photos with GPX…**.

## ExifTool requirement

The plug-in uses ExifTool to read original timezone and subsecond metadata that the Lightroom SDK does not expose.

Windows lookup order:

1. `bin/windows/exiftool.exe`
2. `exiftool.exe` or `exiftool` on `PATH`

macOS lookup order:

1. `bin/macos/exiftool`
2. `/usr/local/bin/exiftool`
3. `/opt/homebrew/bin/exiftool`
4. `exiftool` on `PATH`

On Windows, the official portable ExifTool package also requires its adjacent `exiftool_files` directory.

## Timestamp rules

For `DateTimeOriginal = 2026:07:21 12:00:00` and `OffsetTimeOriginal = -07:00`, the calculated match time is `2026-07-21 19:00:00Z`.

GPX times must contain `Z` or an explicit numeric offset. Timestamps without a timezone are rejected.

## Limitations

- The first release uses the nearest GPX point; it does not interpolate coordinates.
- Conflicting locations with an identical GPX timestamp are rejected as ambiguous.
- Lightroom controls whether catalog GPS changes are later written to DNG/JPEG files or RAW XMP sidecars.
