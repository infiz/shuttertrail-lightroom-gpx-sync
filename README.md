# shuttertrail-lightroom-gpx-sync

By [Shutter Trail](https://infiz.github.io/shuttertrail-pages/).

shuttertrail-lightroom-gpx-sync is a Lightroom Classic plug-in that geotags selected photos by matching their capture times to timestamped points in one or more GPX tracks. It accounts for timezone offsets and subsecond metadata, summarizes the matching results, and updates the Lightroom catalog without modifying original photo files.

## Download

[Download the latest release](https://github.com/infiz/shuttertrail-lightroom-gpx-sync/releases/latest).

## Why use this plug-in?

Lightroom Classic's native GPX track-log workflow does not automatically detect the UTC offsets embedded in individual photos. Instead, it requires the user to provide a manual offset when matching photo capture times to a GPX track. This can be inconvenient and can produce incorrect matches when a selection contains photos with different offsets.

shuttertrail-lightroom-gpx-sync reads embedded photo offsets through ExifTool and reports the offsets it detects. When a photo has no embedded offset, the plug-in asks the user which offset to use and prefills the input with the most-used offset found in the selection. The user can accept the suggestion, change it, choose how broadly to reuse it, or skip photos without offsets.

## Features

- Matches each photo to the nearest GPX point before or after its capture time.
- Reads `DateTimeOriginal`, `SubSecTimeOriginal`, and `OffsetTimeOriginal` through ExifTool.
- Prompts when a photo has no embedded UTC offset and prefills the prompt with the most-used detected offset.
- Lets the user change the suggested offset, reuse it by camera or for the remaining selection, or skip photos without offsets.
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
3. On macOS, open Terminal and run `brew install exiftool`. Windows users can skip this step because ExifTool is bundled.
4. Open Lightroom Classic and choose **File > Plug-In Manager**.
5. Select **Add** in the Plug-In Manager.
6. Browse to and select the `shuttertrail-lightroom-gpx-sync.lrplugin` folder.
7. Confirm that `shuttertrail-lightroom-gpx-sync` appears in the Plug-In Manager and is enabled.

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
4. When a photo has no embedded UTC offset, the plug-in opens an offset prompt. If offsets were detected in other selected photos, the most-used offset is filled in as the suggested value; otherwise the prompt starts with `+00:00`. Accept the suggestion or enter an offset such as `-07:00`, then choose how broadly it should be used:
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

## Standalone desktop application

Development has started on **ShutterTrail Geotagger**, a Windows and macOS desktop application for people who do not use Lightroom Classic. It previews timestamp matches and writes verified GPS metadata directly into supported RAW and JPEG files. The original Lightroom Classic plug-in remains available and unchanged.

The desktop source is under `apps/desktop` and currently supports JPEG, DNG, CR2, CR3, NEF/NRW, ARW, RAF, RW2, ORF, PEF, and SRW as an explicit preview allowlist. Its decisions follow the Lightroom Classic plug-in's Preview flow:

1. Use the single **Add photo files or folders** action, or drag any mixture of photo files and folders into the app. Dropped items are added to the current selection and folders are searched recursively. Discovery skips symbolic links, hidden directories, legacy `.shuttertrail-backups` folders, and visible `ShutterTrail Backups` folders. Select GPX tracks normally or drag `.gpx` files directly onto the GPX card.
2. In Step 03, set the maximum time-difference safety window and choose **Preview matches**. The app shows progress while it loads tracks and metadata. If a photo has no embedded UTC offset, the same prompt as the plug-in suggests the most-used detected offset and can apply it to all remaining photos, the same camera, or only that photo; Skip omits all remaining photos without an offset. Matching then continues automatically with its own progress bar.
3. In the completed preview, choose whether to preserve or replace matched existing locations, just as in the plug-in's Preview dialog, then choose **Apply**. No file is changed before this final action.

Every write creates a byte-for-byte backup in a visible `ShutterTrail Backups/<job-id>` folder beside the source photo before ExifTool modifies the embedded EXIF GPS fields. Each backup keeps the original filename and extension, so the files inside a job folder can be copied directly back into the source folder to restore them.

### Run the desktop app for development

Requirements:

- Rust and Cargo
- Node.js and npm
- ExifTool available on `PATH`, or `SHUTTERTRAIL_EXIFTOOL` set to its executable

```sh
cd apps/desktop
npm install
npm run tauri -- dev
```

### Build installation packages

Windows builds use the ExifTool distribution already bundled with the Lightroom plug-in and produce an NSIS installer:

```bat
scripts\build_windows_package.bat
```

macOS builds stage the Homebrew ExifTool distribution into the application and produce a DMG. The final public package should be built with Developer ID signing and notarization credentials configured in the build environment.

```sh
brew install exiftool
scripts/build_mac_package.sh
```

Set `SHUTTERTRAIL_MAC_TARGET` to a Rust target such as `aarch64-apple-darwin` or `x86_64-apple-darwin` to place the build under that target directory.

Both packaging scripts copy their finished installer into the repository-level `dist` folder. macOS produces a `.dmg`; Windows produces an NSIS `.exe`.

## License

The project is licensed under the [Apache License 2.0](LICENSE). The bundled Windows ExifTool distribution has its own licensing terms; see [`THIRD_PARTY_NOTICES.txt`](shuttertrail-lightroom-gpx-sync.lrplugin/THIRD_PARTY_NOTICES.txt).
