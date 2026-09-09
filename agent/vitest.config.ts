import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
<<<<<<< before updating
    // Runs DROP/CREATE/migrate once at process start. Per-test isolation
    // is provided by `setupTx()` in src/test/db.ts (BEGIN/ROLLBACK), not
    // by re-running migrations or TRUNCATE.
    globalSetup: ['./src/test/global-setup.ts'],
||||||| last update
  })
=======
    // Spelled out (matching Vitest's own default) so knip's static analysis
    // of this file can resolve test entry files; Vitest's own runtime
    // behavior is unchanged.
    include: ['**/*.{test,spec}.?(c|m)[jt]s?(x)'],
>>>>>>> after updating
  },})
