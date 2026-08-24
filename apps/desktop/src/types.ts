export interface DiscoverPhotosRequest {
  roots: string[];
}

export interface DiscoverPhotosResult {
  photoPaths: string[];
  scannedFolderCount: number;
  warnings: string[];
}

export interface SourceReviewRequest {
  photoPaths: string[];
  gpxPaths: string[];
}

export interface ReviewProgress {
  stage: "tracks" | "metadata" | "analysis" | "matching" | "writing" | "complete";
  completed: number;
  total: number;
  percent: number;
  message: string;
  fileName?: string | null;
}

export interface SourceReviewItem {
  path: string;
  fileName: string;
  captureTime: string | null;
  embeddedOffset: string | null;
  utcTime: string | null;
  cameraKey: string;
  cameraLabel: string;
  existingGps: boolean;
  status: "ready" | "missing_offset" | "missing_time" | "unsupported" | "error";
  message: string;
}

export interface OffsetSummaryItem {
  offset: string;
  count: number;
}

export interface SourceReviewResult {
  exiftoolVersion: string;
  photoCount: number;
  supportedPhotoCount: number;
  absoluteTimeCount: number;
  fallbackNeededCount: number;
  missingCaptureTimeCount: number;
  existingGpsCount: number;
  gpxFileCount: number;
  trackPointCount: number;
  trackStartUtc: string | null;
  trackEndUtc: string | null;
  suggestedOffset: string | null;
  offsetSummary: OffsetSummaryItem[];
  items: SourceReviewItem[];
  warnings: string[];
}

export interface PreviewRequest {
  photoPaths: string[];
  gpxPaths: string[];
  offsetAssignments: Record<string, string>;
  maximumDifferenceSeconds: number;
}

export interface PreviewItem {
  path: string;
  fileName: string;
  captureTime: string | null;
  resolvedOffset: string | null;
  utcTime: string | null;
  latitude: number | null;
  longitude: number | null;
  altitude: number | null;
  differenceSeconds: number | null;
  existingGps: boolean;
  status: "matched" | "existing_gps" | "missing_time" | "missing_offset" | "unmatched" | "unsupported" | "error";
  message: string;
}

export interface PreviewResult {
  planId: string;
  exiftoolVersion: string;
  trackPointCount: number;
  matchedCount: number;
  matchedExistingGpsCount: number;
  warningCount: number;
  items: PreviewItem[];
  warnings: string[];
}

export interface ApplyItemResult {
  path: string;
  backupPath: string | null;
  success: boolean;
  message: string;
}

export interface ApplyResult {
  appliedCount: number;
  preservedCount: number;
  failedCount: number;
  items: ApplyItemResult[];
}
