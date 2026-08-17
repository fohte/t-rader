import { defineConfig } from 'tsup'

export default defineConfig({
  entry: ['src/index.ts'],
  format: ['esm'],
  // Keep in sync with the node version in .mise.toml.
  target: 'node24',
  platform: 'node',
  outDir: 'dist',
  clean: true,
  // Bundle first-party code; keep node_modules external so
  // @opentelemetry/auto-instrumentations-node's module-patching hook still
  // applies to the real package in node_modules instead of a bundled copy.
  skipNodeModulesBundle: true,
<<<<<<< before updating
  // skipNodeModulesBundle は相対/絶対パス以外の import をすべて external
  // 扱いにするため、subpath imports (`#foo`) も external 化されてしまう。
  // src を含まない runtime イメージで解決できるよう明示的にバンドルする。
||||||| last update
=======
  // skipNodeModulesBundle treats subpath imports (`#foo`) as external too,
  // leaving `./src/*.ts` specifiers unresolved in a runtime image that only
  // ships dist/. Force-bundle them so dist stays self-contained.
>>>>>>> after updating
  noExternal: [/^#/],
})
