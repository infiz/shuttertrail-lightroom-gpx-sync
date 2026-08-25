import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { buildVersion } from "../../scripts/geotagger_build_version.mjs";

export default defineConfig({
  plugins: [svelte()],
  define: {
    __SHUTTERTRAIL_GEOTAGGER_BUILD_VERSION__: JSON.stringify(buildVersion)
  },
  clearScreen: false,
  server: {
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"]
    }
  }
});
