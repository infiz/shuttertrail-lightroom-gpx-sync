# shuttertrail-lightroom-gpx-sync

By [Shutter Trail](https://infiz.github.io/shuttertrail-pages/).

shuttertrail-lightroom-gpx-sync is a Lightroom Classic plug-in that geotags selected photos by matching their capture times to timestamped points in one or more GPX tracks. It accounts for timezone offsets and subsecond metadata, previews proposed matches, and updates the Lightroom catalog without modifying original photo files.

## Features

- Matches each photo to the nearest GPX point before or after its capture time.
- Reads `DateTimeOriginal`, `SubSecTimeOriginal`, and `OffsetTimeOriginal` through ExifTool.
- Prompts for a UTC offset when a photo does not contain one, with options to reuse it by camera or for the remaining selection.
- Searches across multiple GPX files and accepts matches up to one hour away.
- Previews coordinates, altitude, time difference, and any matching issues before applying changes.
- Preserves existing GPS metadata unless replacement is explicitly selected.
- Shows progress while reading metadata, matching tracks, and updating the catalog.
- Writes latitude, longitude, and available altitude to the Lightroom catalog only.

## Requirements

- Adobe Lightroom Classic with Lightroom SDK 6.0 support or newer.
- ExifTool:
  - Windows: a portable ExifTool runtime is included with the plug-in.
  - macOS: install ExifTool separately and make it available at `/usr/local/bin/exiftool`, `/opt/homebrew/bin/exiftool`, or on `PATH`. You may alternatively place it at `shuttertrail-lightroom-gpx-sync.lrplugin/bin/macos/exiftool`.
- GPX tracks whose timestamps include `Z` or an explicit numeric UTC offset.

## Installation

1. Download or clone this repository.
2. Keep the entire `shuttertrail-lightroom-gpx-sync.lrplugin` folder in a permanent location. Do not move or remove files inside it.
3. In Lightroom Classic, open **File > Plug-In Manager**.
4. Select **Add**, choose the `shuttertrail-lightroom-gpx-sync.lrplugin` folder, and confirm that the plug-in is enabled.

## Usage

1. In Lightroom Classic's Library module, select the photos to geotag.
2. Choose **Library > Plug-in Extras > Sync selected photos with GPX...**.
3. Select one or more GPX files.
4. Supply a UTC offset if prompted for photos that do not contain timezone metadata.
5. Review the proposed matches. Existing GPS data is preserved by default; choose the replacement option only when you intend to overwrite it.
6. Apply the accepted matches to the Lightroom catalog.

Lightroom controls whether catalog GPS changes are subsequently written to JPEG or DNG files, or to XMP sidecars for RAW files.

## Timestamp matching

Photo capture times are converted to UTC before matching. For example:

```text
DateTimeOriginal:   2026:07:21 12:00:00
OffsetTimeOriginal: -07:00
UTC match time:     2026-07-21 19:00:00Z
```

The plug-in finds the closest GPX point across all selected tracks. A match is rejected when it is more than one hour from the photo timestamp. GPX timestamps without timezone information are also rejected.

## Current limitations

- Coordinates are taken from the nearest GPX point; positions are not interpolated.
- Conflicting locations with the same GPX timestamp are treated as ambiguous.
- Video files are ignored.
- A macOS ExifTool binary is not bundled.

## Support

For support, contact [shuttertrail.support@gmail.com](mailto:shuttertrail.support@gmail.com).

## License

The project is licensed under the [Apache License 2.0](LICENSE). The bundled Windows ExifTool distribution has its own licensing terms; see [`THIRD_PARTY_NOTICES.txt`](shuttertrail-lightroom-gpx-sync.lrplugin/THIRD_PARTY_NOTICES.txt).
