import { cp, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { build } from "vite";

const root = fileURLToPath(new URL("./", import.meta.url));
const outDir = resolve(root, "../dist");
const desktop = resolve(outDir, "desktop");
await build({ root, build: { outDir, emptyOutDir: true } });
await cp(resolve(root, "../src/web"), desktop, { recursive: true });
const desktopHtml = resolve(desktop, "index.html");
const markup = await readFile(desktopHtml, "utf8");
await writeFile(desktopHtml, markup.replace('<script src="app.js"></script>', '<script type="module" src="desktop.js"></script>'));
await build({
  root,
  build: {
    outDir: desktop,
    emptyOutDir: false,
    lib: { entry: resolve(root, "desktop.ts"), formats: ["es"], fileName: () => "desktop.js" },
  },
});
