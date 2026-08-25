mod exiftool;
mod gpx;
mod time;

use crate::exiftool::ExifToolManager;
use crate::gpx::{closest, parse_files};
use crate::time::{display_utc, normalize_offset, photo_time_to_utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;
use uuid::Uuid;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "dng", "cr2", "cr3", "nef", "nrw", "arw", "raf", "rw2", "orf", "pef", "srw",
];
const METADATA_BATCH_SIZE: usize = 20;
const BACKUP_DIRECTORY_NAME: &str = "ShutterTrail Backups";

#[derive(Clone)]
struct AppState {
    exiftool: Arc<Mutex<ExifToolManager>>,
    current_plan: Arc<Mutex<Option<WritePlan>>>,
    operation_progress: Arc<Mutex<HashMap<String, ReviewProgress>>>,
}

impl AppState {
    fn new(resource_dir: PathBuf) -> Self {
        Self {
            exiftool: Arc::new(Mutex::new(ExifToolManager::new(resource_dir))),
            current_plan: Arc::new(Mutex::new(None)),
            operation_progress: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn shutdown(&self) {
        if let Ok(mut exiftool) = self.exiftool.lock() {
            exiftool.shutdown();
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewRequest {
    photo_paths: Vec<PathBuf>,
    gpx_paths: Vec<PathBuf>,
    offset_assignments: HashMap<String, String>,
    maximum_difference_seconds: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceReviewRequest {
    photo_paths: Vec<PathBuf>,
    gpx_paths: Vec<PathBuf>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoverPhotosRequest {
    roots: Vec<PathBuf>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscoverPhotosResult {
    photo_paths: Vec<PathBuf>,
    scanned_folder_count: usize,
    warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceReviewResult {
    exiftool_version: String,
    photo_count: usize,
    supported_photo_count: usize,
    absolute_time_count: usize,
    fallback_needed_count: usize,
    missing_capture_time_count: usize,
    existing_gps_count: usize,
    gpx_file_count: usize,
    track_point_count: usize,
    track_start_utc: Option<String>,
    track_end_utc: Option<String>,
    suggested_offset: Option<String>,
    offset_summary: Vec<OffsetSummaryItem>,
    items: Vec<SourceReviewItem>,
    warnings: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewProgress {
    stage: &'static str,
    completed: usize,
    total: usize,
    percent: u8,
    message: String,
    file_name: Option<String>,
}

#[derive(Clone, Copy)]
struct ProgressAmount {
    completed: usize,
    total: usize,
    percent: u8,
}

impl ProgressAmount {
    fn new(completed: usize, total: usize, percent: u8) -> Self {
        Self {
            completed,
            total,
            percent,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceReviewItem {
    path: PathBuf,
    file_name: String,
    capture_time: Option<String>,
    embedded_offset: Option<String>,
    utc_time: Option<String>,
    camera_key: String,
    camera_label: String,
    existing_gps: bool,
    status: &'static str,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OffsetSummaryItem {
    offset: String,
    count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewResult {
    plan_id: String,
    exiftool_version: String,
    track_point_count: usize,
    matched_count: usize,
    matched_existing_gps_count: usize,
    warning_count: usize,
    items: Vec<PreviewItem>,
    warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewItem {
    path: PathBuf,
    file_name: String,
    capture_time: Option<String>,
    resolved_offset: Option<String>,
    utc_time: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    altitude: Option<f64>,
    difference_seconds: Option<f64>,
    existing_gps: bool,
    status: &'static str,
    message: String,
}

struct WritePlanEntry {
    path: PathBuf,
    latitude: f64,
    longitude: f64,
    altitude: Option<f64>,
    existing_gps: bool,
    fingerprint: FileFingerprint,
}

struct WritePlan {
    id: String,
    entries: Vec<WritePlanEntry>,
}

#[derive(PartialEq, Eq)]
struct FileFingerprint {
    size: u64,
    modified_nanos: Option<u128>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyResult {
    applied_count: usize,
    preserved_count: usize,
    failed_count: usize,
    items: Vec<ApplyItemResult>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyItemResult {
    path: PathBuf,
    backup_path: Option<PathBuf>,
    success: bool,
    message: String,
}

#[tauri::command]
async fn discover_photos(request: DiscoverPhotosRequest) -> Result<DiscoverPhotosResult, String> {
    tauri::async_runtime::spawn_blocking(move || discover_photos_blocking(request))
        .await
        .map_err(|error| format!("Photo discovery worker failed: {error}"))?
}

fn discover_photos_blocking(
    request: DiscoverPhotosRequest,
) -> Result<DiscoverPhotosResult, String> {
    if request.roots.is_empty() {
        return Err("Select at least one folder".into());
    }
    Ok(discover_photo_paths(request.roots))
}

fn discover_photo_paths(roots: Vec<PathBuf>) -> DiscoverPhotosResult {
    let mut photo_paths = Vec::new();
    let mut warnings = Vec::new();
    let mut folders = Vec::new();
    let mut scanned_folder_count = 0;

    for root in unique_paths(roots) {
        match fs::symlink_metadata(&root) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                warnings.push(format!("Skipped symbolic link: {}", root.display()))
            }
            Ok(metadata) if metadata.is_dir() => folders.push(root),
            Ok(metadata) if metadata.is_file() && is_supported_photo(&root) => {
                photo_paths.push(root)
            }
            Ok(_) => warnings.push(format!("Skipped unsupported path: {}", root.display())),
            Err(error) => warnings.push(format!("Could not inspect {}: {error}", root.display())),
        }
    }

    while let Some(folder) = folders.pop() {
        scanned_folder_count += 1;
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(error) => {
                warnings.push(format!("Could not read {}: {error}", folder.display()));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    warnings.push(format!(
                        "Could not read an item in {}: {error}",
                        folder.display()
                    ));
                    continue;
                }
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    warnings.push(format!("Could not inspect {}: {error}", path.display()));
                    continue;
                }
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                if !should_skip_discovery_folder(&path) {
                    folders.push(path);
                }
            } else if file_type.is_file() && is_supported_photo(&path) {
                photo_paths.push(path);
            }
        }
    }

    photo_paths.sort_by_key(|path| normalized_path_key(path));
    photo_paths.dedup_by(|left, right| normalized_path_key(left) == normalized_path_key(right));
    DiscoverPhotosResult {
        photo_paths,
        scanned_folder_count,
        warnings,
    }
}

fn should_skip_discovery_folder(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.starts_with('.') || name.eq_ignore_ascii_case(BACKUP_DIRECTORY_NAME)
}

#[tauri::command]
async fn inspect_sources(
    request: SourceReviewRequest,
    operation_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<SourceReviewResult, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        inspect_sources_blocking(request, operation_id, state)
    })
    .await
    .map_err(|error| format!("Source review worker failed: {error}"))?
}

fn inspect_sources_blocking(
    request: SourceReviewRequest,
    operation_id: String,
    state: AppState,
) -> Result<SourceReviewResult, String> {
    if request.photo_paths.is_empty() {
        return Err("Select at least one photo".into());
    }
    if request.gpx_paths.is_empty() {
        return Err("Select at least one GPX file".into());
    }

    let photo_paths = unique_paths(request.photo_paths);
    let gpx_paths = unique_paths(request.gpx_paths);
    let photo_count = photo_paths.len();
    send_operation_progress(
        &state,
        &operation_id,
        "tracks",
        ProgressAmount::new(0, photo_count, 0),
        format!(
            "Reading {} GPX track{}\u{2026}",
            gpx_paths.len(),
            if gpx_paths.len() == 1 { "" } else { "s" }
        ),
        None,
    );
    let (points, warnings) = parse_files(&gpx_paths)?;
    send_operation_progress(
        &state,
        &operation_id,
        "tracks",
        ProgressAmount::new(0, photo_count, 5),
        format!("Loaded {} GPX track points", points.len()),
        None,
    );
    let track_start_utc = points
        .first()
        .and_then(|point| {
            chrono::DateTime::<chrono::Utc>::from_timestamp_millis(point.epoch_millis)
        })
        .map(display_utc);
    let track_end_utc = points
        .last()
        .and_then(|point| {
            chrono::DateTime::<chrono::Utc>::from_timestamp_millis(point.epoch_millis)
        })
        .map(display_utc);

    let mut exiftool = state
        .exiftool
        .lock()
        .map_err(|_| "ExifTool service lock was poisoned".to_string())?;
    let exiftool_version = exiftool.version()?;
    let mut metadata = Vec::with_capacity(photo_paths.len());
    for (batch_index, batch) in photo_paths.chunks(METADATA_BATCH_SIZE).enumerate() {
        let mut batch_metadata = exiftool.read_json(batch)?;
        metadata.append(&mut batch_metadata);
        let completed_photos = ((batch_index + 1) * METADATA_BATCH_SIZE).min(photo_paths.len());
        send_operation_progress(
            &state,
            &operation_id,
            "metadata",
            ProgressAmount::new(
                completed_photos,
                photo_count,
                metadata_scan_percent(completed_photos, photo_count),
            ),
            format!("Scanning photo metadata: {completed_photos} of {photo_count}"),
            None,
        );
    }
    drop(exiftool);
    let metadata_by_path = index_metadata(metadata);
    send_operation_progress(
        &state,
        &operation_id,
        "analysis",
        ProgressAmount::new(photo_count, photo_count, 95),
        "Analyzing timestamps and existing locations…".into(),
        None,
    );

    let mut items = Vec::with_capacity(photo_paths.len());
    let mut offset_counts = HashMap::<String, usize>::new();
    for path in &photo_paths {
        let file_name = display_file_name(path);
        if !is_supported_photo(path) {
            items.push(SourceReviewItem {
                path: path.clone(),
                file_name,
                capture_time: None,
                embedded_offset: None,
                utc_time: None,
                camera_key: String::new(),
                camera_label: "Unknown camera".into(),
                existing_gps: false,
                status: "unsupported",
                message: "This file type has not been enabled for embedded writes".into(),
            });
            continue;
        }

        let Some(object) = metadata_by_path.get(&normalized_path_key(path)) else {
            items.push(SourceReviewItem {
                path: path.clone(),
                file_name,
                capture_time: None,
                embedded_offset: None,
                utc_time: None,
                camera_key: String::new(),
                camera_label: "Unknown camera".into(),
                existing_gps: false,
                status: "error",
                message: "ExifTool did not return metadata for this file".into(),
            });
            continue;
        };

        let capture_time = value_as_string(object.get("DateTimeOriginal"));
        let subsecond = value_as_string(object.get("SubSecTimeOriginal"));
        let embedded_offset = value_as_string(object.get("OffsetTimeOriginal"));
        let normalized_embedded_offset = embedded_offset
            .as_deref()
            .and_then(|value| normalize_offset(value).ok());
        if let Some(offset) = &normalized_embedded_offset {
            *offset_counts.entry(offset.clone()).or_default() += 1;
        }
        let (camera_key, camera_label) = camera_identity(object);
        let existing_gps = value_as_f64(object.get("GPSLatitude")).is_some()
            && value_as_f64(object.get("GPSLongitude")).is_some();
        let Some(capture) = capture_time.as_deref() else {
            items.push(SourceReviewItem {
                path: path.clone(),
                file_name,
                capture_time: None,
                embedded_offset,
                utc_time: None,
                camera_key,
                camera_label,
                existing_gps,
                status: "missing_time",
                message: "DateTimeOriginal is missing; this photo will be skipped".into(),
            });
            continue;
        };

        let Some(offset) = normalized_embedded_offset else {
            let message = if embedded_offset.is_some() {
                "The embedded UTC offset is invalid; provide a fallback or skip this photo"
            } else {
                "No absolute capture time; provide a fallback UTC offset or skip this photo"
            };
            items.push(SourceReviewItem {
                path: path.clone(),
                file_name,
                capture_time,
                embedded_offset,
                utc_time: None,
                camera_key,
                camera_label,
                existing_gps,
                status: "missing_offset",
                message: message.into(),
            });
            continue;
        };

        match photo_time_to_utc(capture, subsecond.as_deref(), &offset) {
            Ok(utc) => items.push(SourceReviewItem {
                path: path.clone(),
                file_name,
                capture_time,
                embedded_offset: Some(offset),
                utc_time: Some(display_utc(utc)),
                camera_key,
                camera_label,
                existing_gps,
                status: "ready",
                message: if existing_gps {
                    "Absolute capture time detected; existing GPS found"
                } else {
                    "Absolute capture time detected"
                }
                .into(),
            }),
            Err(error) => items.push(SourceReviewItem {
                path: path.clone(),
                file_name,
                capture_time,
                embedded_offset: Some(offset),
                utc_time: None,
                camera_key,
                camera_label,
                existing_gps,
                status: "missing_time",
                message: format!("{error}; this photo will be skipped"),
            }),
        }
    }

    let supported_photo_count = items
        .iter()
        .filter(|item| item.status != "unsupported")
        .count();
    let absolute_time_count = items.iter().filter(|item| item.status == "ready").count();
    let fallback_needed_count = items
        .iter()
        .filter(|item| item.status == "missing_offset")
        .count();
    let missing_capture_time_count = items
        .iter()
        .filter(|item| item.status == "missing_time")
        .count();
    let existing_gps_count = items.iter().filter(|item| item.existing_gps).count();
    let mut offset_summary = offset_counts
        .into_iter()
        .map(|(offset, count)| OffsetSummaryItem { offset, count })
        .collect::<Vec<_>>();
    offset_summary.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.offset.cmp(&right.offset))
    });
    let suggested_offset = offset_summary.first().map(|item| item.offset.clone());

    send_operation_progress(
        &state,
        &operation_id,
        "complete",
        ProgressAmount::new(photo_count, photo_count, 100),
        format!(
            "Review complete: {} photo{} checked",
            photo_paths.len(),
            if photo_paths.len() == 1 { "" } else { "s" }
        ),
        None,
    );

    Ok(SourceReviewResult {
        exiftool_version,
        photo_count: photo_paths.len(),
        supported_photo_count,
        absolute_time_count,
        fallback_needed_count,
        missing_capture_time_count,
        existing_gps_count,
        gpx_file_count: gpx_paths.len(),
        track_point_count: points.len(),
        track_start_utc,
        track_end_utc,
        suggested_offset,
        offset_summary,
        items,
        warnings,
    })
}

#[tauri::command]
async fn preview_geotags(
    request: PreviewRequest,
    operation_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<PreviewResult, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        preview_geotags_blocking(request, operation_id, state)
    })
    .await
    .map_err(|error| format!("Match preview worker failed: {error}"))?
}

fn preview_geotags_blocking(
    request: PreviewRequest,
    operation_id: String,
    state: AppState,
) -> Result<PreviewResult, String> {
    if request.photo_paths.is_empty() {
        return Err("Select at least one photo".into());
    }
    if request.gpx_paths.is_empty() {
        return Err("Select at least one GPX file".into());
    }
    if request.maximum_difference_seconds == 0 {
        return Err("Maximum time difference must be greater than zero".into());
    }

    let offset_assignments = request
        .offset_assignments
        .iter()
        .map(|(path, offset)| {
            normalize_offset(offset).map(|offset| (normalized_path_key(Path::new(path)), offset))
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
    let photo_paths = unique_paths(request.photo_paths);
    let gpx_paths = unique_paths(request.gpx_paths);
    let photo_count = photo_paths.len();
    send_operation_progress(
        &state,
        &operation_id,
        "tracks",
        ProgressAmount::new(0, photo_count, 0),
        format!(
            "Loading {} GPX track{} for matching\u{2026}",
            gpx_paths.len(),
            if gpx_paths.len() == 1 { "" } else { "s" }
        ),
        None,
    );
    let (points, warnings) = parse_files(&gpx_paths)?;
    send_operation_progress(
        &state,
        &operation_id,
        "tracks",
        ProgressAmount::new(0, photo_count, 5),
        format!("Loaded {} GPX track points", points.len()),
        None,
    );

    let mut exiftool = state
        .exiftool
        .lock()
        .map_err(|_| "ExifTool service lock was poisoned".to_string())?;
    let exiftool_version = exiftool.version()?;
    let mut metadata = Vec::with_capacity(photo_paths.len());
    for (batch_index, batch) in photo_paths.chunks(METADATA_BATCH_SIZE).enumerate() {
        let mut batch_metadata = exiftool.read_json(batch)?;
        metadata.append(&mut batch_metadata);
        let completed_photos = ((batch_index + 1) * METADATA_BATCH_SIZE).min(photo_paths.len());
        send_operation_progress(
            &state,
            &operation_id,
            "metadata",
            ProgressAmount::new(
                completed_photos,
                photo_count,
                metadata_scan_percent(completed_photos, photo_count),
            ),
            format!("Scanning photo metadata: {completed_photos} of {photo_count}"),
            None,
        );
    }
    drop(exiftool);

    let metadata_by_path = index_metadata(metadata);
    send_operation_progress(
        &state,
        &operation_id,
        "matching",
        ProgressAmount::new(photo_count, photo_count, 95),
        "Calculating nearest GPX matches…".into(),
        None,
    );

    let mut items = Vec::with_capacity(photo_paths.len());
    let mut entries = Vec::new();
    for path in photo_paths {
        let file_name = display_file_name(&path);
        if !is_supported_photo(&path) {
            items.push(empty_item(
                path,
                file_name,
                "unsupported",
                "This file type has not been enabled for embedded writes",
            ));
            continue;
        }

        let Some(object) = metadata_by_path.get(&normalized_path_key(&path)) else {
            items.push(empty_item(
                path,
                file_name,
                "error",
                "ExifTool did not return metadata for this file",
            ));
            continue;
        };
        let capture_time = value_as_string(object.get("DateTimeOriginal"));
        let subsecond = value_as_string(object.get("SubSecTimeOriginal"));
        let embedded_offset = value_as_string(object.get("OffsetTimeOriginal"));
        let latitude = value_as_f64(object.get("GPSLatitude"));
        let longitude = value_as_f64(object.get("GPSLongitude"));
        let existing_gps = latitude.is_some() && longitude.is_some();

        let Some(capture) = capture_time.as_deref() else {
            items.push(PreviewItem {
                path,
                file_name,
                capture_time: None,
                resolved_offset: None,
                utc_time: None,
                latitude: None,
                longitude: None,
                altitude: None,
                difference_seconds: None,
                existing_gps,
                status: "missing_time",
                message: "DateTimeOriginal is missing".into(),
            });
            continue;
        };
        let offset = embedded_offset
            .as_deref()
            .and_then(|value| normalize_offset(value).ok())
            .or_else(|| offset_assignments.get(&normalized_path_key(&path)).cloned());
        let Some(offset) = offset else {
            items.push(PreviewItem {
                path,
                file_name,
                capture_time,
                resolved_offset: None,
                utc_time: None,
                latitude: None,
                longitude: None,
                altitude: None,
                difference_seconds: None,
                existing_gps,
                status: "missing_offset",
                message: "Skipped because no UTC offset was supplied".into(),
            });
            continue;
        };

        let utc = match photo_time_to_utc(capture, subsecond.as_deref(), &offset) {
            Ok(value) => value,
            Err(error) => {
                items.push(PreviewItem {
                    path,
                    file_name,
                    capture_time,
                    resolved_offset: Some(offset),
                    utc_time: None,
                    latitude: None,
                    longitude: None,
                    altitude: None,
                    difference_seconds: None,
                    existing_gps,
                    status: "missing_time",
                    message: error,
                });
                continue;
            }
        };
        let (point, difference) = match closest(
            &points,
            utc.timestamp_millis(),
            request.maximum_difference_seconds,
        ) {
            Ok(value) => value,
            Err(error) => {
                items.push(PreviewItem {
                    path,
                    file_name,
                    capture_time,
                    resolved_offset: Some(offset),
                    utc_time: Some(display_utc(utc)),
                    latitude: None,
                    longitude: None,
                    altitude: None,
                    difference_seconds: None,
                    existing_gps,
                    status: "unmatched",
                    message: error,
                });
                continue;
            }
        };
        let fingerprint = fingerprint(&path)?;
        entries.push(WritePlanEntry {
            path: path.clone(),
            latitude: point.latitude,
            longitude: point.longitude,
            altitude: point.altitude,
            existing_gps,
            fingerprint,
        });
        items.push(PreviewItem {
            path,
            file_name,
            capture_time,
            resolved_offset: Some(offset),
            utc_time: Some(display_utc(utc)),
            latitude: Some(point.latitude),
            longitude: Some(point.longitude),
            altitude: point.altitude,
            difference_seconds: Some(difference),
            existing_gps,
            status: "matched",
            message: if existing_gps {
                format!("Matched from {}; existing GPS found", point.source)
            } else {
                format!("Matched from {}", point.source)
            },
        });
    }

    let plan_id = Uuid::new_v4().simple().to_string();
    let plan = WritePlan {
        id: plan_id.clone(),
        entries,
    };
    let matched_count = plan.entries.len();
    let matched_existing_gps_count = plan
        .entries
        .iter()
        .filter(|entry| entry.existing_gps)
        .count();
    let warning_count =
        warnings.len() + items.iter().filter(|item| item.status != "matched").count();
    *state
        .current_plan
        .lock()
        .map_err(|_| "Write plan lock was poisoned".to_string())? = Some(plan);

    send_operation_progress(
        &state,
        &operation_id,
        "complete",
        ProgressAmount::new(photo_count, photo_count, 100),
        format!(
            "Match preview complete: {matched_count} photo{} matched",
            if matched_count == 1 { "" } else { "s" }
        ),
        None,
    );

    Ok(PreviewResult {
        plan_id,
        exiftool_version,
        track_point_count: points.len(),
        matched_count,
        matched_existing_gps_count,
        warning_count,
        items,
        warnings,
    })
}

#[tauri::command]
async fn apply_geotags(
    plan_id: String,
    operation_id: String,
    replace_existing_gps: bool,
    state: tauri::State<'_, AppState>,
) -> Result<ApplyResult, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let plan = {
            let mut current_plan = state
                .current_plan
                .lock()
                .map_err(|_| "Write plan lock was poisoned".to_string())?;
            if current_plan.as_ref().map(|plan| plan.id.as_str()) != Some(plan_id.as_str()) {
                return Err("This preview is no longer available. Create a new preview.".into());
            }
            current_plan.take().ok_or_else(|| {
                "This preview is no longer available. Create a new preview.".to_string()
            })?
        };
        let total_work = plan.entries.len().max(1);
        send_operation_progress(
            &state,
            &operation_id,
            "writing",
            ProgressAmount::new(0, total_work, 0),
            "Preparing the first verified backup…".into(),
            None,
        );
        apply_geotags_blocking(plan, operation_id, replace_existing_gps, state)
    })
    .await
    .map_err(|error| format!("Apply worker failed: {error}"))?
}

fn apply_geotags_blocking(
    plan: WritePlan,
    operation_id: String,
    replace_existing_gps: bool,
    state: AppState,
) -> Result<ApplyResult, String> {
    let total_work = plan.entries.len().max(1);

    let mut exiftool = state
        .exiftool
        .lock()
        .map_err(|_| "ExifTool service lock was poisoned".to_string())?;
    let mut items = Vec::with_capacity(plan.entries.len());
    let mut preserved_count = 0;
    for (entry_index, entry) in plan.entries.into_iter().enumerate() {
        let file_name = display_file_name(&entry.path);
        if entry.existing_gps && !replace_existing_gps {
            preserved_count += 1;
            send_operation_progress(
                &state,
                &operation_id,
                "writing",
                ProgressAmount::new(
                    entry_index + 1,
                    total_work,
                    completed_percent(entry_index + 1, total_work),
                ),
                "Preserved existing GPS".into(),
                Some(file_name),
            );
            continue;
        }
        let result = apply_entry(&plan.id, entry, &mut exiftool, |message| {
            send_operation_progress(
                &state,
                &operation_id,
                "writing",
                ProgressAmount::new(
                    entry_index,
                    total_work,
                    completed_percent(entry_index, total_work),
                ),
                message.into(),
                Some(file_name.clone()),
            );
        });
        let status = if result.success {
            "Updated"
        } else {
            "Could not update"
        };
        items.push(result);
        send_operation_progress(
            &state,
            &operation_id,
            "writing",
            ProgressAmount::new(
                entry_index + 1,
                total_work,
                completed_percent(entry_index + 1, total_work),
            ),
            status.into(),
            Some(file_name),
        );
    }
    let applied_count = items.iter().filter(|item| item.success).count();
    let failed_count = items.len() - applied_count;
    send_operation_progress(
        &state,
        &operation_id,
        "complete",
        ProgressAmount::new(total_work, total_work, 100),
        format!(
            "Apply complete: {applied_count} updated, {preserved_count} preserved, {failed_count} failed"
        ),
        None,
    );
    Ok(ApplyResult {
        applied_count,
        preserved_count,
        failed_count,
        items,
    })
}

#[tauri::command]
fn get_operation_progress(
    operation_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<ReviewProgress>, String> {
    state
        .operation_progress
        .lock()
        .map_err(|_| "Operation progress lock was poisoned".to_string())
        .map(|progress| progress.get(&operation_id).cloned())
}

#[tauri::command]
fn clear_operation_progress(
    operation_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .operation_progress
        .lock()
        .map_err(|_| "Operation progress lock was poisoned".to_string())?
        .remove(&operation_id);
    Ok(())
}

#[tauri::command]
fn quit_application(app: tauri::AppHandle) {
    app.exit(0);
}

fn send_operation_progress(
    state: &AppState,
    operation_id: &str,
    stage: &'static str,
    amount: ProgressAmount,
    message: String,
    file_name: Option<String>,
) {
    let progress = ReviewProgress {
        stage,
        completed: amount.completed,
        total: amount.total,
        percent: amount.percent,
        message,
        file_name,
    };
    if let Ok(mut snapshots) = state.operation_progress.lock() {
        snapshots.insert(operation_id.to_string(), progress);
    }
}

fn completed_percent(completed: usize, total: usize) -> u8 {
    if total == 0 {
        return 100;
    }
    completed.saturating_mul(100).saturating_div(total).min(100) as u8
}

fn metadata_scan_percent(completed: usize, total: usize) -> u8 {
    if total == 0 {
        return 95;
    }
    5 + completed.saturating_mul(90).saturating_div(total).min(90) as u8
}

fn apply_entry(
    plan_id: &str,
    entry: WritePlanEntry,
    exiftool: &mut ExifToolManager,
    mut on_progress: impl FnMut(&str),
) -> ApplyItemResult {
    let path = entry.path.clone();
    let mut created_backup = None;
    let result = (|| -> Result<(PathBuf, String), String> {
        on_progress("Checking that the photo has not changed");
        if fingerprint(&path)? != entry.fingerprint {
            return Err("File changed after preview; no metadata was written".into());
        }
        if fs::symlink_metadata(&path)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("Symbolic links are not modified".into());
        }

        on_progress("Creating and verifying the backup");
        let parent = path
            .parent()
            .ok_or_else(|| "Photo has no parent directory".to_string())?;
        let name = path
            .file_name()
            .ok_or_else(|| "Photo has no filename".to_string())?;
        let backup_dir = parent.join(BACKUP_DIRECTORY_NAME).join(plan_id);
        fs::create_dir_all(&backup_dir)
            .map_err(|error| format!("Could not create {}: {error}", backup_dir.display()))?;
        let backup_path = backup_dir.join(name);
        fs::copy(&path, &backup_path)
            .map_err(|error| format!("Could not create backup: {error}"))?;
        created_backup = Some(backup_path.clone());
        if sha256(&path)? != sha256(&backup_path)? {
            return Err("Backup verification failed; original was not modified".into());
        }

        on_progress("Writing embedded GPS metadata");
        if let Err(error) =
            exiftool.write_gps(&path, entry.latitude, entry.longitude, entry.altitude)
        {
            fs::copy(&backup_path, &path).map_err(|restore| {
                format!("ExifTool write failed: {error}; backup restore also failed: {restore}")
            })?;
            return Err(format!(
                "ExifTool write failed: {error}; original restored from backup"
            ));
        }

        on_progress("Verifying embedded GPS metadata");
        if let Err(error) = verify_gps(exiftool, &path, entry.latitude, entry.longitude) {
            fs::copy(&backup_path, &path)
                .map_err(|restore| format!("{error}; backup restore also failed: {restore}"))?;
            return Err(format!("{error}; original restored from backup"));
        }
        Ok((backup_path, "Embedded GPS written and verified".into()))
    })();

    match result {
        Ok((backup_path, message)) => ApplyItemResult {
            path,
            backup_path: Some(backup_path),
            success: true,
            message,
        },
        Err(message) => ApplyItemResult {
            path,
            backup_path: created_backup,
            success: false,
            message,
        },
    }
}

fn verify_gps(
    exiftool: &mut ExifToolManager,
    path: &Path,
    latitude: f64,
    longitude: f64,
) -> Result<(), String> {
    let values = exiftool.read_json(&[path.to_path_buf()])?;
    let object = values
        .first()
        .and_then(Value::as_object)
        .ok_or_else(|| "ExifTool returned no verification metadata".to_string())?;
    let actual_latitude = value_as_f64(object.get("GPSLatitude"))
        .ok_or_else(|| "Written GPS latitude was not readable".to_string())?;
    let actual_longitude = value_as_f64(object.get("GPSLongitude"))
        .ok_or_else(|| "Written GPS longitude was not readable".to_string())?;
    if (actual_latitude - latitude).abs() > 0.000001
        || (actual_longitude - longitude).abs() > 0.000001
    {
        return Err(format!(
            "GPS verification mismatch: expected {latitude:.7},{longitude:.7}; read {actual_latitude:.7},{actual_longitude:.7}"
        ));
    }
    Ok(())
}

fn empty_item(
    path: PathBuf,
    file_name: String,
    status: &'static str,
    message: &str,
) -> PreviewItem {
    PreviewItem {
        path,
        file_name,
        capture_time: None,
        resolved_offset: None,
        utc_time: None,
        latitude: None,
        longitude: None,
        altitude: None,
        difference_seconds: None,
        existing_gps: false,
        status,
        message: message.into(),
    }
}

fn index_metadata(metadata: Vec<Value>) -> HashMap<String, serde_json::Map<String, Value>> {
    let mut metadata_by_path = HashMap::new();
    for value in metadata {
        if let Some(object) = value.as_object()
            && let Some(source) = value_as_string(object.get("SourceFile"))
        {
            metadata_by_path.insert(normalized_path_key(Path::new(&source)), object.clone());
        }
    }
    metadata_by_path
}

fn display_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Unknown photo")
        .to_string()
}

fn is_supported_photo(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    SUPPORTED_EXTENSIONS.contains(&extension.as_str())
}

fn camera_identity(object: &serde_json::Map<String, Value>) -> (String, String) {
    let make = value_as_string(object.get("Make")).unwrap_or_default();
    let model = value_as_string(object.get("Model")).unwrap_or_default();
    let serial = value_as_string(object.get("SerialNumber"))
        .or_else(|| value_as_string(object.get("BodySerialNumber")))
        .or_else(|| value_as_string(object.get("InternalSerialNumber")))
        .unwrap_or_default();
    let label = format!("{} {}", make.trim(), model.trim())
        .trim()
        .to_string();
    (
        format!("{make}|{model}|{serial}"),
        if label.is_empty() {
            "Unknown camera".into()
        } else {
            label
        },
    )
}

fn unique_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(normalized_path_key(path)))
        .collect()
}

fn normalized_path_key(path: &Path) -> String {
    let value = path.to_string_lossy().to_string();
    if cfg!(target_os = "windows") {
        value.replace('\\', "/").to_ascii_lowercase()
    } else {
        value
    }
}

fn value_as_string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn value_as_f64(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(value) => value.as_f64(),
        Value::String(value) => value.parse().ok(),
        _ => None,
    }
}

fn fingerprint(path: &Path) -> Result<FileFingerprint, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Could not inspect {}: {error}", path.display()))?;
    Ok(FileFingerprint {
        size: metadata.len(),
        modified_nanos: metadata.modified().ok().and_then(system_time_nanos),
    })
}

fn system_time_nanos(value: SystemTime) -> Option<u128> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_nanos())
}

fn sha256(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 1024 * 128];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest.finalize().to_vec())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let resource_dir = app.path().resource_dir()?;
            app.manage(AppState::new(resource_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            discover_photos,
            inspect_sources,
            preview_geotags,
            apply_geotags,
            get_operation_progress,
            clear_operation_progress,
            quit_application
        ])
        .build(tauri::generate_context!())
        .expect("error while building ShutterTrail Geotagger");

    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            app_handle.state::<AppState>().shutdown();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempTree(PathBuf);

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn recursively_discovers_supported_photos_and_skips_private_folders() {
        let root = std::env::temp_dir().join(format!(
            "shuttertrail-discovery-test-{}",
            Uuid::new_v4().simple()
        ));
        let _guard = TempTree(root.clone());
        fs::create_dir_all(root.join("nested/deeper")).unwrap();
        fs::create_dir_all(root.join(".hidden")).unwrap();
        fs::create_dir_all(root.join(".shuttertrail-backups/job")).unwrap();
        fs::create_dir_all(root.join("ShutterTrail Backups/job")).unwrap();
        fs::write(root.join("one.JPG"), b"one").unwrap();
        fs::write(root.join("nested/two.nef"), b"two").unwrap();
        fs::write(root.join("nested/deeper/three.txt"), b"three").unwrap();
        fs::write(root.join(".hidden/hidden.jpg"), b"hidden").unwrap();
        fs::write(root.join(".shuttertrail-backups/job/backup.jpg"), b"backup").unwrap();
        fs::write(
            root.join("ShutterTrail Backups/job/visible-backup.jpg"),
            b"backup",
        )
        .unwrap();

        let result = discover_photo_paths(vec![root.clone(), root]);
        let names = result
            .photo_paths
            .iter()
            .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["two.nef", "one.JPG"]);
        assert_eq!(result.scanned_folder_count, 3);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn progress_snapshots_are_operation_scoped() {
        let state = AppState::new(PathBuf::new());
        send_operation_progress(
            &state,
            "operation-a",
            "writing",
            ProgressAmount::new(7, 12, 58),
            "Writing photo 8".into(),
            Some("IMG_0008.CR3".into()),
        );

        let snapshots = state.operation_progress.lock().unwrap();
        let progress = snapshots.get("operation-a").unwrap();
        assert_eq!(progress.completed, 7);
        assert_eq!(progress.total, 12);
        assert_eq!(progress.percent, 58);
        assert_eq!(progress.message, "Writing photo 8");
        assert_eq!(progress.file_name.as_deref(), Some("IMG_0008.CR3"));
        assert!(!snapshots.contains_key("operation-b"));
    }

    #[test]
    fn metadata_scanning_uses_the_first_ninety_five_percent() {
        assert_eq!(metadata_scan_percent(0, 100), 5);
        assert_eq!(metadata_scan_percent(50, 100), 50);
        assert_eq!(metadata_scan_percent(100, 100), 95);
        assert_eq!(completed_percent(7, 12), 58);
        assert_eq!(completed_percent(12, 12), 100);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn matches_exiftool_windows_paths_with_forward_slashes() {
        let selected_path = Path::new(r"C:\Users\Photographer\Pictures\DSC08891.ARW");
        let exiftool_path = Path::new("C:/Users/Photographer/Pictures/DSC08891.ARW");

        assert_eq!(
            normalized_path_key(selected_path),
            normalized_path_key(exiftool_path)
        );
    }
}
