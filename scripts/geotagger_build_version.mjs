import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const appDirectory = resolve(scriptDirectory, "../apps/desktop");

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

export const buildVersion = readFileSync(resolve(appDirectory, "BUILD_VERSION"), "utf8").trim();

if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(buildVersion)) {
  throw new Error(`Invalid ShutterTrail GeoTagger build version: ${buildVersion || "<empty>"}`);
}

const versionedFiles = [
  ["apps/desktop/package.json", readJson(resolve(appDirectory, "package.json")).version],
  ["apps/desktop/src-tauri/tauri.conf.json", readJson(resolve(appDirectory, "src-tauri/tauri.conf.json")).version]
];

for (const [file, version] of versionedFiles) {
  if (version !== buildVersion) {
    throw new Error(`${file} uses version ${version}; update it to match BUILD_VERSION (${buildVersion}).`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  process.stdout.write(`${buildVersion}\n`);
}
