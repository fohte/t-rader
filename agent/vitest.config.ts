import { defineConfig } from 'vitest/config'

export default defineConfig({
  resolve: {
    alias: {
      '@': '/src',
    },
  },
  test: {
    // Runs DROP/CREATE/migrate once at process start. Per-test isolation
    // is provided by `setupTx()` in src/test/db.ts (BEGIN/ROLLBACK), not
    // by re-running migrations or TRUNCATE.
    globalSetup: ['./src/test/global-setup.ts'],
  },
})
