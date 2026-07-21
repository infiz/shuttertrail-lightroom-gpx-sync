# shuttertrail-lightroom-gpx-sync

By [Shutter Trail](https://infiz.github.io/shuttertrail-pages/).

<<<<<<< HEAD
shuttertrail-lightroom-gpx-sync is a Lightroom Classic plug-in that geotags selected photos by matching their capture times to timestamped points in one or more GPX tracks. It accounts for timezone offsets and subsecond metadata, summarizes the matching results, and updates the Lightroom catalog without modifying original photo files.

## Why use this plug-in?

Lightroom Classic's native GPX track-log workflow does not automatically detect the UTC offsets embedded in individual photos. Instead, it requires the user to provide a manual offset when matching photo capture times to a GPX track. This can be inconvenient and can produce incorrect matches when a selection contains photos with different offsets.

shuttertrail-lightroom-gpx-sync reads embedded photo offsets through ExifTool, reports the offsets it detects, and automatically fills missing photo offsets with the most-used offset found in the selection. This reduces the amount of offset information the user must enter manually. Manual offset entry is required only when none of the selected photos provides a usable offset.
||||||| parent of ae72682 (Suggest detected offsets for missing metadata)
shuttertrail-lightroom-gpx-sync is a Lightroom Classic plug-in that geotags selected photos by matching their capture times to timestamped points in one or more GPX tracks. It accounts for timezone offsets and subsecond metadata, previews proposed matches, and updates the Lightroom catalog without modifying original photo files.
=======
shuttertrail-lightroom-gpx-sync is a Lightroom Classic plug-in that geotags selected photos by matching their capture times to timestamped points in one or more GPX tracks. It accounts for timezone offsets and subsecond metadata, summarizes the matching results, and updates the Lightroom catalog without modifying original photo files.

## Download

[Download the latest `main` branch as a ZIP](https://github.com/infiz/shuttertrail-lightroom-gpx-sync/archive/refs/heads/main.zip).

## Why use this plug-in?

Lightroom Classic's native GPX track-log workflow does not automatically detect the UTC offsets embedded in individual photos. Instead, it requires the user to provide a manual offset when matching photo capture times to a GPX track. This can be inconvenient and can produce incorrect matches when a selection contains photos with different offsets.

shuttertrail-lightroom-gpx-sync reads embedded photo offsets through ExifTool and reports the offsets it detects. When a photo has no embedded offset, the plug-in asks the user which offset to use and prefills the input with the most-used offset found in the selection. The user can accept the suggestion, change it, choose how broadly to reuse it, or skip photos without offsets.
>>>>>>> ae72682 (Suggest detected offsets for missing metadata)

## Features

- Matches each photo to the nearest GPX point before or after its capture time.
- Reads `DateTimeOriginal`, `SubSecTimeOriginal`, and `OffsetTimeOriginal` through ExifTool.
<<<<<<< HEAD
- Automatically fills missing photo offsets with the most-used embedded UTC offset detected in the selected photos.
- Prompts for a UTC offset when none of the selected photos contains one, with options to reuse it by camera or for the remaining selection.
||||||| parent of ae72682 (Suggest detected offsets for missing metadata)
- Prompts for a UTC offset when a photo does not contain one, with options to reuse it by camera or for the remaining selection.
=======
- Prompts when a photo has no embedded UTC offset and prefills the prompt with the most-used detected offset.
- Lets the user change the suggested offset, reuse it by camera or for the remaining selection, or skip photos without offsets.
>>>>>>> ae72682 (Suggest detected offsets for missing metadata)
- Searches across multiple GPX files and accepts matches up to one hour away.
- Shows summary statistics for the matching results before applying changes.
- Preserves existing GPS metadata unless replacement is explicitly selected.
- Shows progress while reading metadata, matching tracks, and updating the catalog.
- Writes latitude, longitude, and available altitude to the Lightroom catalog only.

## Requirements

- Adobe Lightroom Classic with Lightroom SDK 6.0 support or newer.
- ExifTool:
  - Windows: a portable ExifTool runtime is included with the plug-in.
  - macOS: install [ExifTool with Homebrew](https://formulae.brew.sh/formula/exiftool) before setting up the plug-in:

    ```sh
    brew install exiftool
    ```

    If Homebrew is not installed, install it from [brew.sh](https://brew.sh/) first.
- GPX tracks whose timestamps include `Z` or an explicit numeric UTC offset.

## Set up the plug-in

1. Download or clone this repository.
2. Keep the entire `shuttertrail-lightroom-gpx-sync.lrplugin` folder in a permanent location. The plug-in needs the code and bundled support files inside this folder, so do not move or remove them individually.
<<<<<<< HEAD
3. On macOS, open Terminal and run `brew install exiftool`. Windows users can skip this step because ExifTool is bundled.
4. Open Lightroom Classic and choose **File > Plug-In Manager**.
5. Select **Add** in the Plug-In Manager.
6. Browse to and select the `shuttertrail-lightroom-gpx-sync.lrplugin` folder.
7. Confirm that `shuttertrail-lightroom-gpx-sync` appears in the Plug-In Manager and is enabled.
||||||| parent of ae72682 (Suggest detected offsets for missing metadata)
3. Open Lightroom Classic and choose **File > Plug-In Manager**.
4. Select **Add** in the Plug-In Manager.
5. Browse to and select the `shuttertrail-lightroom-gpx-sync.lrplugin` folder.
6. Confirm that `shuttertrail-lightroom-gpx-sync` appears in the Plug-In Manager and is enabled.
=======
3. On macOS, open Terminal and run `brew install exiftool`. Windows users can skip this step because ExifTool is bundled.
4. Open Lightroom Classic and choose **File > Plug-In Manager**.
5. Select **Add** in the Plug-In Manager.
6. Browse to and select the `shuttertrail-lightroom-gpx-sync.lrplugin` folder.
7. Confirm that `shuttertrail-lightroom-gpx-sync` appears in the Plug-In Manager and is enabled.

### Enable pre-commit checks

Contributors can enable the repository's [pre-commit](https://pre-commit.com/) checks after cloning. If the earlier standalone hook was enabled, remove its Git setting first, then install the framework hook:

```sh
git config --unset core.hooksPath || true
pre-commit install
```

Run the check against every file at any time with:

```sh
pre-commit run --all-files
```

The check automatically converts CRLF and mixed line endings to LF in text files. Binary files are skipped. When files are corrected, review the changes, stage them, and run the check again.
>>>>>>> ae72682 (Suggest detected offsets for missing metadata)

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
<<<<<<< HEAD
4. If some photos have embedded UTC offsets, the plug-in uses the most frequently detected offset for photos without one. The results summary lists every detected offset and its photo count. If no selected photo has an embedded offset, enter an offset such as `-07:00`, then choose how broadly it should be used:
||||||| parent of ae72682 (Suggest detected offsets for missing metadata)
4. If a selected photo has no embedded UTC offset, enter an offset such as `-07:00`, then choose how broadly it should be used:
=======
4. When a photo has no embedded UTC offset, the plug-in opens an offset prompt. If offsets were detected in other selected photos, the most-used offset is filled in as the suggested value; otherwise the prompt starts with `+00:00`. Accept the suggestion or enter an offset such as `-07:00`, then choose how broadly it should be used:
>>>>>>> ae72682 (Suggest detected offsets for missing metadata)
   - **All remaining photos without an embedded offset** applies it to every remaining photo that needs an offset.
   - **Remaining photos from this camera** applies it only to remaining photos from the same camera.
   - **This photo only** applies it once and prompts again for the next photo without an offset.
   - **Skip All Photos Without Offset** leaves the current and all remaining photos without embedded offsets unmatched.
5. Review the matching statistics, including selected-file counts, offset counts, matched-photo counts, and existing-location counts. Individual photo matches are not shown.
6. Existing GPS data is preserved by default. Enable the replacement option only if you intend to overwrite it.
7. Confirm the results summary to apply the matches to the Lightroom catalog, or cancel without making catalog changes.

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
