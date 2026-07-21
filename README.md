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

## Set up the plug-in

1. Download or clone this repository.
2. Keep the entire `shuttertrail-lightroom-gpx-sync.lrplugin` folder in a permanent location. The plug-in needs the code and bundled support files inside this folder, so do not move or remove them individually.
3. Open Lightroom Classic and choose **File > Plug-In Manager**.
4. Select **Add** in the Plug-In Manager.
5. Browse to and select the `shuttertrail-lightroom-gpx-sync.lrplugin` folder.
6. Confirm that `shuttertrail-lightroom-gpx-sync` appears in the Plug-In Manager and is enabled.

### Reload after an upgrade

1. Close any open shuttertrail-lightroom-gpx-sync dialogs.
2. Replace the existing plug-in files with the files from the new version. Keep the upgraded folder at the same location and retain the name `shuttertrail-lightroom-gpx-sync.lrplugin`.
3. In Lightroom Classic, choose **File > Plug-In Manager**.
4. Select `shuttertrail-lightroom-gpx-sync` in the list.
5. Select **Reload Plug-in**. If Lightroom does not show the updated version, restart Lightroom Classic.

## Use the plug-in

1. In Lightroom Classic's Library module, select the photos you want to geotag. Video files in the selection are ignored.
2. Choose **Library > Plug-in Extras > Sync selected photos with GPX...**.
3. In the file picker, select one or more `.gpx` files and choose **Use GPX Files**. The plug-in searches all supplied files for the closest timestamped point.
4. If a selected photo has no embedded UTC offset, enter an offset such as `-07:00`, then choose how broadly it should be used:
   - **All remaining photos without an embedded offset** applies it to every remaining photo that needs an offset.
   - **Remaining photos from this camera** applies it only to remaining photos from the same camera.
   - **This photo only** applies it once and prompts again for the next photo without an offset.
   - **Skip All Photos Without Offset** leaves the current and all remaining photos without embedded offsets unmatched.
5. Review the match preview, including coordinates, altitude, time difference, and any unmatched photos.
6. Existing GPS data is preserved by default. Enable the replacement option only if you intend to overwrite it.
7. Confirm the preview to apply the accepted matches to the Lightroom catalog, or cancel without making catalog changes.

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
