import { defineConfig } from "astro/config";

const base = process.env.SITE_BASE ?? "/";

export default defineConfig({
  site: process.env.SITE_URL ?? "https://xmeta.eidos.space",
  base,
  output: "static",
  i18n: {
    defaultLocale: "en",
    locales: ["en", "zh"],
    routing: { prefixDefaultLocale: false },
  },
  build: {
    format: "directory",
  },
});
