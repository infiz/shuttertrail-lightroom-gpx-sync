<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { PhysicalPosition } from "@tauri-apps/api/dpi";
  import { confirm, open } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type {
    ApplyResult,
    DiscoverPhotosRequest,
    DiscoverPhotosResult,
    PreviewRequest,
    PreviewResult,
    ReviewProgress,
    SourceReviewRequest,
    SourceReviewItem,
    SourceReviewResult
  } from "./types";

  const productPageUrl = "https://infiz.github.io/shuttertrail-pages/";
  const iosAppStoreUrl = "https://apps.apple.com/us/app/shutter-trail/id6792596736";

  let photoPaths: string[] = [];
  let photoSelectionNote = "";
  let gpxPaths: string[] = [];
  let gpxSelectionNote = "";
  let sourceReview: SourceReviewResult | null = null;
  let offsetAssignments: Record<string, string> = {};
  let unresolvedOffsetItems: SourceReviewItem[] = [];
  let offsetPromptItem: SourceReviewItem | null = null;
  let offsetPromptValue = "+00:00";
  let offsetPromptScope: "all" | "camera" | "one" = "all";
  let offsetPromptError = "";
  let offsetResolutionComplete = false;
  let skippedOffsetCount = 0;
  let maximumMinutes = 60;
  let existingGpsPolicy: "skip" | "overwrite" = "skip";
  let preview: PreviewResult | null = null;
  let hasPreviewed = false;
  let applyResult: ApplyResult | null = null;
  let busy = false;
  let busyAction: "scan" | "review" | "match" | "write" | null = null;
  let activity = "Select photos and one or more GPX tracks to begin.";
  let errorMessage = "";
  let showPhotoSourceMenu = false;
  let dragActive = false;
  let dragOverGpx = false;
  let gpxDropArea: HTMLElement | null = null;
  let reviewProgress: ReviewProgress | null = null;
  let previewProgress: ReviewProgress | null = null;
  let applyProgress: ReviewProgress | null = null;
  let operationProgress: ReviewProgress;
  let operationTitle = "Applying embedded GPS";
  let operationCountLabel = "photos processed";
  let operationHint = "Each photo is backed up and verified before continuing.";

  $: operationProgress = (busyAction === "review"
    ? reviewProgress
    : busyAction === "match"
      ? previewProgress
      : busyAction === "write"
        ? applyProgress
        : null) ?? { stage: "analysis", completed: 0, total: 1, percent: 0, message: activity };
  $: operationTitle = busyAction === "review"
    ? "Reviewing selected files"
    : busyAction === "match"
      ? "Building match preview"
      : "Applying embedded GPS";
  $: operationCountLabel = busyAction === "write" ? "photos processed" : "photos scanned";
  $: operationHint = busyAction === "write"
    ? "Each photo is backed up and verified before continuing."
    : "Keep ShutterTrail open while the selected files are checked.";

  const photoExtensions = [
    "jpg", "jpeg", "dng", "cr2", "cr3", "nef", "nrw", "arw", "raf", "rw2", "orf", "pef", "srw"
  ];

  async function choosePhotos() {
    showPhotoSourceMenu = false;
    const selected = await open({
      multiple: true,
      directory: false,
      title: "Choose photos to geotag",
      filters: [{ name: "Supported photos", extensions: photoExtensions }]
    });
    if (selected) await loadPhotoRoots(Array.isArray(selected) ? selected : [selected], true, "files");
  }

  async function choosePhotoFolders() {
    showPhotoSourceMenu = false;
    const selected = await open({
      multiple: true,
      directory: true,
      title: "Choose folders containing photos"
    });
    if (selected) await loadPhotoRoots(Array.isArray(selected) ? selected : [selected], true, "folders");
  }

  async function loadPhotoRoots(roots: string[], mergeExisting: boolean, origin: "files" | "folders" | "drop") {
    if (!roots.length || busy) return;
    const previousCount = photoPaths.length;
    const scanRoots = mergeExisting ? [...photoPaths, ...roots] : roots;
    busy = true;
    busyAction = "scan";
    errorMessage = "";
    activity = origin === "folders"
      ? `Scanning ${roots.length} selected folder${roots.length === 1 ? "" : "s"} recursively…`
      : origin === "drop"
        ? `Inspecting ${roots.length} dropped item${roots.length === 1 ? "" : "s"}…`
        : `Adding ${roots.length} selected photo${roots.length === 1 ? "" : "s"}…`;
    try {
      const request: DiscoverPhotosRequest = { roots: scanRoots };
      const result = await invoke<DiscoverPhotosResult>("discover_photos", { request });
      photoPaths = result.photoPaths;
      const addedCount = Math.max(0, result.photoPaths.length - (mergeExisting ? previousCount : 0));
      photoSelectionNote = origin === "folders"
        ? `${result.photoPaths.length} supported photo${result.photoPaths.length === 1 ? "" : "s"} found across ${result.scannedFolderCount} folder${result.scannedFolderCount === 1 ? "" : "s"}.`
        : origin === "drop"
          ? `${result.photoPaths.length} photos selected; ${addedCount} added by the last drop.`
          : `${result.photoPaths.length} individual photo file${result.photoPaths.length === 1 ? "" : "s"} selected.`;
      if (addedCount > 0) resetSourceReview();
      if (result.photoPaths.length === 0) {
        errorMessage = "No supported photo files were found in the selected items.";
        activity = "Selection completed without finding supported photos.";
      } else {
        activity = `${origin === "drop" ? "Drop" : "Selection"} complete: ${photoSelectionNote}${result.warnings.length ? ` ${result.warnings.length} warning${result.warnings.length === 1 ? "" : "s"}.` : ""}`;
      }
    } catch (error) {
      errorMessage = String(error);
      activity = "The photo selection could not be scanned.";
    } finally {
      busy = false;
      busyAction = null;
    }
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWindow().onDragDropEvent(({ payload }) => {
      if (payload.type === "enter" || payload.type === "over") {
        dragActive = !busy && !offsetPromptItem;
        dragOverGpx = dragActive && isPointInside(payload.position, gpxDropArea);
      } else if (payload.type === "leave") {
        dragActive = false;
        dragOverGpx = false;
      } else if (payload.type === "drop") {
        const droppedOnGpx = isPointInside(payload.position, gpxDropArea);
        dragActive = false;
        dragOverGpx = false;
        if (!busy && !offsetPromptItem) {
          if (droppedOnGpx) addDroppedGpx(payload.paths);
          else void loadPhotoRoots(payload.paths, true, "drop");
        }
      }
    }).then((stop) => unlisten = stop);
    return () => unlisten?.();
  });

  function isPointInside(position: PhysicalPosition, element: HTMLElement | null) {
    if (!element) return false;
    const logical = position.toLogical(window.devicePixelRatio || 1);
    const bounds = element.getBoundingClientRect();
    return logical.x >= bounds.left && logical.x <= bounds.right
      && logical.y >= bounds.top && logical.y <= bounds.bottom;
  }

  function addDroppedGpx(paths: string[]) {
    const accepted = paths.filter((path) => /\.gpx$/i.test(path));
    if (!accepted.length) {
      errorMessage = "Drop one or more .gpx files into the GPX area.";
      activity = "The GPX drop did not contain a GPX file.";
      return;
    }
    const before = gpxPaths.length;
    gpxPaths = Array.from(new Set([...gpxPaths, ...accepted]));
    const added = gpxPaths.length - before;
    const ignored = paths.length - accepted.length;
    gpxSelectionNote = `${gpxPaths.length} GPX file${gpxPaths.length === 1 ? "" : "s"} selected; ${added} added by the last drop.`;
    resetSourceReview();
    activity = `GPX drop complete: ${gpxSelectionNote}${ignored ? ` ${ignored} non-GPX item${ignored === 1 ? " was" : "s were"} ignored.` : ""}`;
  }

  async function chooseGpx() {
    const selected = await open({
      multiple: true,
      directory: false,
      title: "Choose GPX tracks",
      filters: [{ name: "GPX tracks", extensions: ["gpx"] }]
    });
    if (selected) {
      const additions = Array.isArray(selected) ? selected : [selected];
      const previousCount = gpxPaths.length;
      gpxPaths = Array.from(new Set([...gpxPaths, ...additions]));
      const addedCount = gpxPaths.length - previousCount;
      gpxSelectionNote = `${gpxPaths.length} GPX file${gpxPaths.length === 1 ? "" : "s"} selected; ${addedCount} added.`;
      if (addedCount > 0) resetSourceReview();
      activity = addedCount
        ? `Added ${addedCount} GPX file${addedCount === 1 ? "" : "s"}. Preview again to review the updated selection.`
        : "Those GPX files were already selected.";
    }
  }

  function clearPhotos() {
    if (busy || !photoPaths.length) return;
    photoPaths = [];
    photoSelectionNote = "";
    showPhotoSourceMenu = false;
    resetSourceReview();
    activity = "Photo selection cleared. Add photos to continue.";
  }

  function clearGpx() {
    if (busy || !gpxPaths.length) return;
    gpxPaths = [];
    gpxSelectionNote = "";
    resetSourceReview();
    activity = "GPX selection cleared. Add one or more tracks to continue.";
  }

  function removeGpx(path: string) {
    if (busy) return;
    gpxPaths = gpxPaths.filter((item) => item !== path);
    gpxSelectionNote = gpxPaths.length
      ? `${gpxPaths.length} GPX file${gpxPaths.length === 1 ? "" : "s"} selected.`
      : "";
    resetSourceReview();
    activity = gpxPaths.length
      ? "GPX file removed. Preview again to review the updated selection."
      : "The last GPX file was removed. Add a track to continue.";
  }

  function resetSourceReview() {
    sourceReview = null;
    offsetAssignments = {};
    unresolvedOffsetItems = [];
    offsetPromptItem = null;
    offsetResolutionComplete = false;
    skippedOffsetCount = 0;
    existingGpsPolicy = "skip";
    resetMatchPreview();
    activity = "Selections changed. Review the files before choosing match behavior.";
  }

  function resetMatchPreview() {
    preview = null;
    applyResult = null;
    errorMessage = "";
    if (sourceReview && offsetResolutionComplete) activity = "Preview settings changed. Build the match preview again.";
  }

  function startNewSession() {
    if (busy) return;
    photoPaths = [];
    photoSelectionNote = "";
    gpxPaths = [];
    gpxSelectionNote = "";
    sourceReview = null;
    offsetAssignments = {};
    unresolvedOffsetItems = [];
    offsetPromptItem = null;
    offsetPromptValue = "+00:00";
    offsetPromptScope = "all";
    offsetPromptError = "";
    offsetResolutionComplete = false;
    skippedOffsetCount = 0;
    maximumMinutes = 60;
    existingGpsPolicy = "skip";
    preview = null;
    hasPreviewed = false;
    applyResult = null;
    errorMessage = "";
    showPhotoSourceMenu = false;
    dragActive = false;
    dragOverGpx = false;
    reviewProgress = null;
    previewProgress = null;
    applyProgress = null;
    activity = "New session ready. Select photos and one or more GPX tracks to begin.";
  }

  function quitApplication() {
    if (!busy) void invoke("quit_application");
  }

  async function inspectSources() {
    if (!photoPaths.length || !gpxPaths.length) return;
    busy = true;
    busyAction = "review";
    errorMessage = "";
    preview = null;
    applyResult = null;
    reviewProgress = {
      stage: "tracks",
      completed: 0,
      total: photoPaths.length,
      percent: 0,
      message: "Preparing the file review…"
    };
    activity = reviewProgress.message;
    await paintOperationOverlay();
    const operationId = crypto.randomUUID();
    const stopProgress = startOperationProgressPolling(operationId, (progress) => {
      reviewProgress = progress;
      activity = progress.message;
    });
    try {
      const request: SourceReviewRequest = { photoPaths, gpxPaths };
      const review = await invoke<SourceReviewResult>("inspect_sources", { request, operationId });
      sourceReview = review;
      beginOffsetResolution(review);
    } catch (error) {
      sourceReview = null;
      errorMessage = String(error);
      activity = "The selected files could not be reviewed.";
    } finally {
      stopProgress();
      busy = false;
      busyAction = null;
      reviewProgress = null;
    }
  }

  function reviewProgressPercent(progress: ReviewProgress) {
    return Math.min(100, Math.max(0, Math.round(progress.percent)));
  }

  async function paintOperationOverlay() {
    await tick();
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  }

  function startOperationProgressPolling(operationId: string, receive: (progress: ReviewProgress) => void) {
    let stopped = false;
    let timer: number | undefined;
    const pullProgress = async () => {
      try {
        const progress = await invoke<ReviewProgress | null>("get_operation_progress", { operationId });
        if (progress && !stopped) receive(progress);
      } catch {
        // The operation result remains authoritative if a transient progress poll fails.
      } finally {
        if (!stopped) timer = window.setTimeout(() => void pullProgress(), 200);
      }
    };
    void pullProgress();
    return () => {
      stopped = true;
      if (timer !== undefined) window.clearTimeout(timer);
      void invoke("clear_operation_progress", { operationId }).catch(() => {
        // Progress cleanup must not replace the operation result.
      });
    };
  }

  function beginOffsetResolution(review: SourceReviewResult) {
    offsetAssignments = {};
    skippedOffsetCount = 0;
    unresolvedOffsetItems = review.items.filter((item) => item.status === "missing_offset");
    offsetResolutionComplete = unresolvedOffsetItems.length === 0;
    if (offsetResolutionComplete) {
      offsetPromptItem = null;
      activity = `Review complete: all ${review.absoluteTimeCount} readable capture times are absolute. Building matches…`;
      queueMatchPreview();
      return;
    }
    showNextOffsetPrompt();
  }

  function showNextOffsetPrompt() {
    offsetPromptItem = unresolvedOffsetItems[0] ?? null;
    if (!offsetPromptItem) {
      offsetResolutionComplete = true;
      activity = `Offset review complete: ${Object.keys(offsetAssignments).length} assigned, ${skippedOffsetCount} skipped. Building matches…`;
      queueMatchPreview();
      return;
    }
    offsetPromptValue = sourceReview?.suggestedOffset ?? "+00:00";
    offsetPromptScope = unresolvedOffsetItems.length > 1 ? "all" : "one";
    offsetPromptError = "";
    activity = `Waiting for a UTC offset decision for ${offsetPromptItem.fileName}.`;
  }

  function normalizePromptOffset(value: string) {
    const trimmed = value.trim();
    if (trimmed.toLowerCase() === "z") return "+00:00";
    const match = trimmed.match(/^([+-])(\d{2}):?(\d{2})$/);
    if (!match) return null;
    const hours = Number(match[2]);
    const minutes = Number(match[3]);
    if (hours > 14 || minutes > 59 || (hours === 14 && minutes !== 0)) return null;
    return `${match[1]}${match[2]}:${match[3]}`;
  }

  function useOffset() {
    if (!offsetPromptItem) return;
    const normalized = normalizePromptOffset(offsetPromptValue);
    if (!normalized) {
      offsetPromptError = "UTC offset must look like -07:00 or +05:30 and be within ±14:00.";
      return;
    }

    const current = offsetPromptItem;
    const applicable = unresolvedOffsetItems.filter((item, index) =>
      offsetPromptScope === "all"
        || (offsetPromptScope === "camera" && item.cameraKey === current.cameraKey)
        || (offsetPromptScope === "one" && index === 0)
    );
    const appliedPaths = new Set(applicable.map((item) => item.path));
    const assignments = { ...offsetAssignments };
    for (const item of applicable) assignments[item.path] = normalized;
    offsetAssignments = assignments;
    unresolvedOffsetItems = unresolvedOffsetItems.filter((item) => !appliedPaths.has(item.path));
    resetMatchPreview();
    showNextOffsetPrompt();
  }

  function skipAllMissingOffsets() {
    skippedOffsetCount += unresolvedOffsetItems.length;
    unresolvedOffsetItems = [];
    offsetPromptItem = null;
    offsetResolutionComplete = true;
    resetMatchPreview();
    activity = `Offset review complete: ${Object.keys(offsetAssignments).length} assigned, ${skippedOffsetCount} skipped. Building matches…`;
    queueMatchPreview();
  }

  function restartOffsetResolution() {
    if (!sourceReview) return;
    resetMatchPreview();
    beginOffsetResolution(sourceReview);
  }

  function queueMatchPreview() {
    window.setTimeout(() => void buildPreview(), 0);
  }

  async function buildPreview() {
    if (!sourceReview || !offsetResolutionComplete || busy) return;
    busy = true;
    busyAction = "match";
    errorMessage = "";
    applyResult = null;
    previewProgress = {
      stage: "tracks",
      completed: 0,
      total: photoPaths.length,
      percent: 0,
      message: "Preparing the match preview…"
    };
    activity = previewProgress.message;
    await paintOperationOverlay();
    const operationId = crypto.randomUUID();
    const stopProgress = startOperationProgressPolling(operationId, (progress) => {
      previewProgress = progress;
      activity = progress.message;
    });
    try {
      const request: PreviewRequest = {
        photoPaths,
        gpxPaths,
        offsetAssignments,
        maximumDifferenceSeconds: Math.max(1, maximumMinutes) * 60
      };
      preview = await invoke<PreviewResult>("preview_geotags", { request, operationId });
      hasPreviewed = true;
      activity = `Match preview ready: ${preview.matchedCount} photo${preview.matchedCount === 1 ? "" : "s"} can be geotagged.`;
    } catch (error) {
      errorMessage = String(error);
      activity = "The match preview could not be completed.";
    } finally {
      stopProgress();
      busy = false;
      busyAction = null;
      previewProgress = null;
    }
  }

  function readyToWriteCount() {
    if (!preview) return 0;
    return preview.matchedCount - (existingGpsPolicy === "skip" ? preview.matchedExistingGpsCount : 0);
  }

  async function applyGeotags() {
    const writeCount = readyToWriteCount();
    if (!preview || writeCount === 0) return;
    const overwriteCount = existingGpsPolicy === "overwrite" ? preview.matchedExistingGpsCount : 0;
    const overwriteNotice = overwriteCount
      ? `\n\n${overwriteCount} existing GPS location${overwriteCount === 1 ? "" : "s"} will be overwritten.`
      : "";
    const approved = await confirm(
      `Write embedded GPS metadata to ${writeCount} photo${writeCount === 1 ? "" : "s"}?${overwriteNotice}\n\nA byte-for-byte backup with the original filename will be kept in a visible “ShutterTrail Backups” folder beside each photo.`,
      { title: "Write GPS metadata", kind: "warning" }
    );
    if (!approved) return;

    busy = true;
    busyAction = "write";
    errorMessage = "";
    applyProgress = {
      stage: "writing",
      completed: 0,
      total: Math.max(1, preview.matchedCount),
      percent: 0,
      message: "Preparing verified backups…"
    };
    activity = applyProgress.message;
    await paintOperationOverlay();
    const operationId = crypto.randomUUID();
    const stopProgress = startOperationProgressPolling(operationId, (progress) => {
      applyProgress = progress;
      activity = progress.message;
    });
    try {
      applyResult = await invoke<ApplyResult>("apply_geotags", {
        planId: preview.planId,
        operationId,
        replaceExistingGps: existingGpsPolicy === "overwrite"
      });
      activity = `Finished: ${applyResult.appliedCount} updated, ${applyResult.preservedCount} preserved, ${applyResult.failedCount} failed.`;
    } catch (error) {
      errorMessage = String(error);
      activity = "The write operation could not be completed.";
    } finally {
      stopProgress();
      busy = false;
      busyAction = null;
      applyProgress = null;
    }
  }

  function statusLabel(status: string) {
    return status.replaceAll("_", " ");
  }

  function shortPath(path: string) {
    const parts = path.split(/[\\/]/);
    return parts.length > 3 ? `…/${parts.slice(-3).join("/")}` : path;
  }

  async function openExternal(url: string) {
    try {
      await openUrl(url);
    } catch (error) {
      errorMessage = `Could not open the link: ${String(error)}`;
    }
  }
</script>

<svelte:head>
  <title>ShutterTrail Geotagger</title>
</svelte:head>

<main>
  <header class="hero">
    <div class="brand-mark" aria-hidden="true">
      <span></span><span></span><span></span>
    </div>
    <div>
      <p class="eyebrow">SHUTTERTRAIL</p>
      <h1>ShutterTrail Geotagger</h1>
      <p class="lede">Match photo times to GPX tracks, review the results, then write embedded GPS.</p>
    </div>
    <div class="direct-write-note">
      <strong>Embedded metadata</strong>
      <span>No catalog. No sidecars.</span>
    </div>
  </header>

  <nav class="progress-strip" aria-label="Geotagging progress">
    <span class:complete={photoPaths.length > 0 && gpxPaths.length > 0}><i>{photoPaths.length && gpxPaths.length ? "✓" : "1"}</i>Select</span>
    <b>→</b>
    <span class:active={photoPaths.length > 0 && gpxPaths.length > 0 && !preview} class:complete={!!preview}><i>{preview ? "✓" : "2"}</i>Preview matches</span>
    <b>→</b>
    <span class:active={!!preview && !applyResult} class:complete={!!applyResult}><i>{applyResult ? "✓" : "3"}</i>Apply</span>
  </nav>

  <section class="workspace">
    <div class="setup-column">
      {#if !sourceReview || !offsetResolutionComplete}
        <article class="panel selection-panel">
        <div class="panel-heading">
          <span class="step-number">01</span>
          <div><h2>Choose photographs</h2><p>Select files directly or scan one or more folders recursively.</p></div>
        </div>
        <button class="picker" on:click={() => showPhotoSourceMenu = !showPhotoSourceMenu} disabled={busy} aria-expanded={showPhotoSourceMenu}>
          <span>{busyAction === "scan" ? "Scanning photos…" : "Add photo files or folders"}</span><span class="arrow">⌄</span>
        </button>
        {#if showPhotoSourceMenu}
          <div class="source-choice-menu">
            <button type="button" on:click={choosePhotos}><strong>Choose individual files</strong><span>Select one or more RAW or JPEG photos.</span></button>
            <button type="button" on:click={choosePhotoFolders}><strong>Choose folders recursively</strong><span>Find supported photos in every subfolder.</span></button>
          </div>
        {/if}
        <div class="drop-hint" class:active={dragActive && !dragOverGpx}><strong>Or drop photo files and folders here</strong><span>Mixed drops are supported and added to the current selection.</span></div>
        {#if photoPaths.length}
          <div class="file-list">
            <div class="file-list-toolbar">
              <strong>{photoSelectionNote || `${photoPaths.length} photos selected.`}</strong>
              <button type="button" on:click={clearPhotos} disabled={busy}>Clear photos</button>
            </div>
            {#each photoPaths.slice(0, 4) as path}<span title={path}>{shortPath(path)}</span>{/each}
            {#if photoPaths.length > 4}<span>+ {photoPaths.length - 4} more</span>{/if}
          </div>
        {/if}
        </article>

        <article class="panel selection-panel gpx-drop-area" class:drag-target={dragOverGpx} bind:this={gpxDropArea}>
        <div class="panel-heading">
          <span class="step-number">02</span>
          <div><h2>Add GPX tracks</h2><p>The review checks timestamps and coverage before matching.</p></div>
        </div>
        <button class="picker" on:click={chooseGpx} disabled={busy}>
          <span>{gpxPaths.length ? `${gpxPaths.length} selected` : "Select GPX files"}</span>
          <span class="arrow">↗</span>
        </button>
        <div class="drop-hint gpx-drop-hint" class:active={dragOverGpx}><strong>Or drop GPX files here</strong><span>Dropped tracks are added to the current GPX selection.</span></div>
        {#if gpxPaths.length}
          <div class="file-list">
            <div class="file-list-toolbar">
              <strong>{gpxSelectionNote || `${gpxPaths.length} GPX files selected.`}</strong>
              <button type="button" on:click={clearGpx} disabled={busy}>Clear GPX</button>
            </div>
            <div class="selected-files-scroll">
              {#each gpxPaths as path}
                <div class="selected-file-row">
                  <span title={path}>{shortPath(path)}</span>
                  <button type="button" on:click={() => removeGpx(path)} disabled={busy} aria-label={`Remove ${shortPath(path)}`}>Remove</button>
                </div>
              {/each}
            </div>
          </div>
        {/if}
        </article>

        <article class="panel review-action-panel">
        <div class="panel-heading">
          <span class="step-number">03</span>
          <div><h2>Preview matches</h2><p>Review metadata, resolve missing offsets, and calculate GPX matches.</p></div>
        </div>
        <label class="review-setting">
          <span>Maximum time difference</span>
          <div class="input-with-unit"><input type="number" min="1" max="1440" bind:value={maximumMinutes} on:input={resetMatchPreview} /><b>minutes</b></div>
          <small class="field-help">A photo is not matched when the nearest GPX point falls outside this window.</small>
        </label>
        <button class="primary full-width" on:click={inspectSources} disabled={busy || !photoPaths.length || !gpxPaths.length}>
          {busyAction === "review" ? "Reading files…" : hasPreviewed ? "Preview again" : "Preview matches"}
        </button>
        </article>
      {:else}
        <article class="panel completed-panel">
          <div class="completed-heading"><span aria-hidden="true">✓</span><div><h2>{preview ? "Match preview ready" : "Preparing match preview"}</h2><p>Missing offsets are resolved before matching</p></div></div>
          <div class="completed-counts">
            <span><strong>{photoPaths.length}</strong> photos</span>
            <span><strong>{gpxPaths.length}</strong> GPX file{gpxPaths.length === 1 ? "" : "s"}</span>
          </div>
          <label class="completed-setting">
            <span><strong>Step 03 match window</strong><small>Maximum time difference</small></span>
            <div class="compact-input"><input type="number" min="1" max="1440" bind:value={maximumMinutes} on:input={resetMatchPreview} /><b>min</b></div>
          </label>
          <div class="completed-actions">
            <button type="button" on:click={() => showPhotoSourceMenu = !showPhotoSourceMenu} disabled={busy}>Add photos</button>
            <button type="button" on:click={clearPhotos} disabled={busy || !photoPaths.length} class="destructive-action">Clear photos</button>
            <button type="button" on:click={chooseGpx} disabled={busy}>Add GPX files</button>
            <button type="button" on:click={clearGpx} disabled={busy || !gpxPaths.length} class="destructive-action">Clear GPX files</button>
          </div>
          {#if showPhotoSourceMenu}
            <div class="source-choice-menu compact-menu">
              <button type="button" on:click={choosePhotos}><strong>Add individual files</strong></button>
              <button type="button" on:click={choosePhotoFolders}><strong>Add folders recursively</strong></button>
            </div>
          {/if}
          {#if gpxPaths.length}
            <div class="compact-selected-files" aria-label="Selected GPX files">
              {#each gpxPaths as path}
                <div class="selected-file-row">
                  <span title={path}>{shortPath(path)}</span>
                  <button type="button" on:click={() => removeGpx(path)} disabled={busy} aria-label={`Remove ${shortPath(path)}`}>Remove</button>
                </div>
              {/each}
            </div>
          {/if}
          <button class="primary full-width preview-again-cta" type="button" on:click={inspectSources} disabled={busy}>
            {busyAction === "review" ? "Reading files…" : busyAction === "match" ? "Matching…" : "Preview again"}
          </button>
        </article>
      {/if}

    </div>

    <article class="panel results-panel">
      <div class="results-heading">
        <div>
          <p class="eyebrow">{preview ? "MATCH PREVIEW" : busyAction === "match" ? "MATCHING" : sourceReview ? "PREPARING PREVIEW" : "PRE-WRITE PREVIEW"}</p>
          <h2>{preview ? `${readyToWriteCount()} ready to write` : busyAction === "match" ? (previewProgress?.message ?? "Building match preview…") : sourceReview ? `${sourceReview.photoCount} photos reviewed` : "Nothing changes before preview"}</h2>
        </div>
        {#if sourceReview}<span class="engine">ExifTool {sourceReview.exiftoolVersion}</span>{/if}
      </div>

      {#if !sourceReview}
        <div class="empty-state">
          <div class="route-line"><i></i><i></i><i></i></div>
          <p>Select photos and GPX files, then choose Preview matches. Missing UTC offsets are prompted first; existing-location choices appear with the completed match preview.</p>
        </div>
      {:else if !preview}
        <div class="summary-grid">
          <div><strong>{sourceReview.photoCount}</strong><span>photos</span></div>
          <div><strong>{sourceReview.absoluteTimeCount}</strong><span>absolute times</span></div>
          <div><strong>{sourceReview.existingGpsCount}</strong><span>existing GPS</span></div>
          <div><strong>{sourceReview.trackPointCount}</strong><span>track points</span></div>
        </div>
        <div class="track-review">
          <div><strong>{sourceReview.gpxFileCount} GPX file{sourceReview.gpxFileCount === 1 ? "" : "s"}</strong><span>{sourceReview.trackStartUtc ?? "Unknown start"} → {sourceReview.trackEndUtc ?? "Unknown end"}</span></div>
          <div class="review-alerts">
            {#if sourceReview.fallbackNeededCount && !offsetResolutionComplete}<span class="review-warning">Offset decisions required for {sourceReview.fallbackNeededCount} photo{sourceReview.fallbackNeededCount === 1 ? "" : "s"}</span>{/if}
            {#if sourceReview.fallbackNeededCount && offsetResolutionComplete}<span class="review-warning resolved-warning">{Object.keys(offsetAssignments).length} user offsets · {skippedOffsetCount} skipped</span>{/if}
            {#if sourceReview.warnings.length}<span class="review-warning">{sourceReview.warnings.length} GPX file warning{sourceReview.warnings.length === 1 ? "" : "s"}</span>{/if}
          </div>
        </div>
        <div class="table-wrap review-table">
          <table>
            <thead><tr><th>Photo</th><th>Camera capture</th><th>Absolute UTC</th><th>GPS</th><th>Review</th></tr></thead>
            <tbody>
              {#each sourceReview.items as item}
                <tr>
                  <td><strong>{item.fileName}</strong><small title={item.path}>{shortPath(item.path)}</small></td>
                  <td>{item.captureTime ?? "—"}<small>{item.embeddedOffset ?? "No embedded offset"}</small></td>
                  <td>{item.utcTime ?? "—"}</td>
                  <td>{item.existingGps ? "Existing location" : "None"}</td>
                  <td>
                    {#if item.status === "missing_offset" && offsetAssignments[item.path]}
                      <span class="status matched">user offset</span><small>Using {offsetAssignments[item.path]}</small>
                    {:else if item.status === "missing_offset" && offsetResolutionComplete}
                      <span class="status">skipped</span><small>No UTC offset supplied</small>
                    {:else}
                      <span class:matched={item.status === "ready"} class="status">{statusLabel(item.status)}</span><small>{item.message}</small>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        <div class="review-footer">
          <div>
            <strong>{offsetResolutionComplete ? "Building the match preview." : "Offset decisions in progress."}</strong>
            <span>{offsetResolutionComplete ? "The preview will open automatically when matching finishes." : "Resolve the missing offsets in the prompt before matching."}</span>
          </div>
        </div>
      {:else}
        <div class="summary-grid">
          <div><strong>{preview.trackPointCount}</strong><span>track points</span></div>
          <div><strong>{preview.matchedCount}</strong><span>GPX matches total</span></div>
          <div><strong>{preview.matchedExistingGpsCount}</strong><span>matches already with GPS</span></div>
          <div><strong>{preview.warningCount}</strong><span>warnings</span></div>
        </div>
        <div class="preview-options">
          <div class="preview-options-heading">
            <div><strong>Existing locations</strong><span>Choose what Apply should do, just like the Lightroom Classic preview.</span></div>
            {#if sourceReview.fallbackNeededCount > 0}
              <button type="button" on:click={restartOffsetResolution} disabled={busy || !!applyResult}>Change offset choices</button>
            {/if}
          </div>
          {#if preview.matchedExistingGpsCount > 0}
            <div class="preview-choice-grid">
              <label class="choice-card" class:selected={existingGpsPolicy === "skip"}>
                <input type="radio" bind:group={existingGpsPolicy} value="skip" disabled={!!applyResult} />
                <span><strong>Preserve existing locations</strong><small>Update {preview.matchedCount - preview.matchedExistingGpsCount} matched photo{preview.matchedCount - preview.matchedExistingGpsCount === 1 ? "" : "s"}; leave {preview.matchedExistingGpsCount} existing location{preview.matchedExistingGpsCount === 1 ? "" : "s"} unchanged.</small></span>
              </label>
              <label class="choice-card danger-choice" class:selected={existingGpsPolicy === "overwrite"}>
                <input type="radio" bind:group={existingGpsPolicy} value="overwrite" disabled={!!applyResult} />
                <span><strong>Replace existing locations</strong><small>Update all {preview.matchedCount} matched photos from the GPX tracks.</small></span>
              </label>
            </div>
          {:else}
            <div class="decision-note"><strong>No matched photo has an existing location.</strong><span>All {preview.matchedCount} matches can be applied without replacing GPS metadata.</span></div>
          {/if}
          {#if sourceReview.fallbackNeededCount > 0}
            <div class="offset-result"><strong>Missing-offset choices</strong><span>{Object.keys(offsetAssignments).length} assigned · {skippedOffsetCount} skipped</span></div>
          {/if}
        </div>
        <div class="table-wrap">
          <table>
            <thead><tr><th>Photo</th><th>UTC capture</th><th>Coordinates</th><th>Difference</th><th>Status</th></tr></thead>
            <tbody>
              {#each preview.items as item}
                <tr>
                  <td><strong>{item.fileName}</strong><small title={item.path}>{shortPath(item.path)}</small></td>
                  <td>{item.utcTime ?? "—"}<small>{item.resolvedOffset ?? "No offset"}</small></td>
                  <td>{item.latitude === null ? "—" : `${item.latitude.toFixed(5)}, ${item.longitude?.toFixed(5)}`}</td>
                  <td>{item.differenceSeconds === null ? "—" : `${item.differenceSeconds.toFixed(1)}s`}</td>
                  <td><span class:matched={item.status === "matched"} class="status">{statusLabel(item.status)}</span><small>{item.message}</small></td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        {#if applyResult}
          <div class="apply-summary" class:warning={applyResult.failedCount > 0} role="status">
            <div>
              <strong>{applyResult.failedCount > 0 ? "Apply finished with errors" : "GPS metadata applied successfully"}</strong>
              <span>{applyResult.appliedCount} updated · {applyResult.preservedCount} preserved · {applyResult.failedCount} failed</span>
            </div>
            {#if applyResult.failedCount > 0}
              <details>
                <summary>Review failed photos</summary>
                <div class="apply-failures">
                  {#each applyResult.items.filter((item) => !item.success) as item}
                    <p><strong>{shortPath(item.path)}</strong><span>{item.message}</span></p>
                  {/each}
                </div>
              </details>
            {:else}
              <span>Backups keep the original filenames, so they can be copied directly over the modified photos.</span>
            {/if}
          </div>
        {/if}
        {#if applyResult}
          <div class="write-bar completion-bar">
            <div><strong>What would you like to do next?</strong><span>Start a fresh geotagging session or close ShutterTrail.</span></div>
            <div class="completion-buttons">
              <button class="secondary" type="button" on:click={quitApplication}>Quit ShutterTrail</button>
              <button class="primary write" type="button" on:click={startNewSession}>Geotag more photos</button>
            </div>
          </div>
        {:else}
          <div class="write-bar">
            <div><strong>{readyToWriteCount()} photo{readyToWriteCount() === 1 ? "" : "s"} will be updated.</strong><span>Verified backups keep the original filenames for direct restoration.</span></div>
            <button class="primary write" on:click={applyGeotags} disabled={busy || readyToWriteCount() === 0}>
              {busyAction === "write" ? "Writing…" : `Apply to ${readyToWriteCount()} photo${readyToWriteCount() === 1 ? "" : "s"}`}
            </button>
          </div>
        {/if}
      {/if}
    </article>
  </section>

  {#if offsetPromptItem}
    <div class="modal-backdrop" role="presentation">
      <form class="offset-modal" on:submit|preventDefault={useOffset} aria-labelledby="offset-dialog-title">
        <p class="eyebrow">MISSING UTC OFFSET</p>
        <h2 id="offset-dialog-title">No absolute time was found in this photo.</h2>
        <p class="modal-progress">Photo {sourceReview ? sourceReview.fallbackNeededCount - unresolvedOffsetItems.length + 1 : 1} of {sourceReview?.fallbackNeededCount ?? 1} requiring an offset</p>

        <dl class="photo-facts">
          <div><dt>Photo</dt><dd>{offsetPromptItem.fileName}</dd></div>
          <div><dt>Camera</dt><dd>{offsetPromptItem.cameraLabel}</dd></div>
          <div><dt>Capture time</dt><dd>{offsetPromptItem.captureTime ?? "Unavailable"}</dd></div>
        </dl>

        <label class="modal-field">
          <span>UTC offset</span>
          <input bind:value={offsetPromptValue} on:input={() => offsetPromptError = ""} placeholder="-07:00" />
          <small>Example: -07:00</small>
        </label>
        {#if sourceReview?.suggestedOffset}
          <p class="suggestion">Suggested <strong>{sourceReview.suggestedOffset}</strong>, the most-used offset detected in the other photos ({sourceReview.offsetSummary[0].count} photo{sourceReview.offsetSummary[0].count === 1 ? "" : "s"}).</p>
        {:else}
          <p class="suggestion">No embedded offsets were detected, so the prompt starts at +00:00.</p>
        {/if}

        <label class="modal-field">
          <span>Use for</span>
          <select bind:value={offsetPromptScope}>
            <option value="all">All remaining photos without an embedded offset</option>
            <option value="camera">Remaining photos from this camera</option>
            <option value="one">This photo only</option>
          </select>
        </label>
        {#if offsetPromptError}<p class="modal-error">{offsetPromptError}</p>{/if}

        <div class="modal-actions">
          <button class="secondary" type="button" on:click={skipAllMissingOffsets}>Skip all photos without offset</button>
          <button class="primary" type="submit">Use offset</button>
        </div>
      </form>
    </div>
  {/if}

  {#if busyAction === "review" || busyAction === "match" || busyAction === "write"}
    <div class="operation-backdrop" role="dialog" aria-modal="true" aria-labelledby="operation-title" aria-describedby="operation-status">
      <div class="operation-card">
        <div class="operation-symbol" aria-hidden="true"><span></span><span></span><span></span></div>
        <p class="eyebrow">SHUTTERTRAIL IS WORKING</p>
        <h2 id="operation-title">{operationTitle}</h2>
        {#if busyAction === "write" && operationProgress.fileName}
          <div class="operation-current-file">
            <span>Current photo</span>
            <strong title={operationProgress.fileName ?? ""}>{operationProgress.fileName}</strong>
          </div>
        {/if}
        <p id="operation-status" class="operation-status" aria-live="polite">{operationProgress.message}</p>
        <div class="operation-progress-label">
          <span>{operationProgress.completed} of {operationProgress.total} {operationCountLabel}</span>
          <strong>{reviewProgressPercent(operationProgress)}%</strong>
        </div>
        <div class="operation-progress-track" role="progressbar" aria-label={operationTitle} aria-valuemin="0" aria-valuemax="100" aria-valuenow={reviewProgressPercent(operationProgress)}>
          <span style={`width: ${reviewProgressPercent(operationProgress)}%`}></span>
        </div>
        <small>{operationHint}</small>
      </div>
    </div>
  {/if}

  {#if dragActive}
    <div class="drag-overlay" class:gpx-target={dragOverGpx} aria-hidden="true">
      {#if dragOverGpx}
        <div><strong>Drop GPX files</strong><span>The tracks will be added to the current GPX selection.</span></div>
      {:else}
        <div><strong>Drop photos or folders</strong><span>Folders will be searched recursively. Existing selections will be kept.</span></div>
      {/if}
    </div>
  {/if}

  <footer>
    <div class="footer-status" class:error={!!errorMessage}>
      <span class="activity-dot" class:busy></span>
      <p>{errorMessage || activity}</p>
    </div>
    <nav class="footer-links" aria-label="ShutterTrail links">
      <button type="button" on:click={() => openExternal(productPageUrl)} aria-label="Open the ShutterTrail product page in your browser">
        Product page <span aria-hidden="true">↗</span>
      </button>
      <button type="button" on:click={() => openExternal(iosAppStoreUrl)} aria-label="Open Shutter Trail for iOS in the App Store">
        iOS App Store <span aria-hidden="true">↗</span>
      </button>
    </nav>
  </footer>
</main>
