import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    include: ['tests/frontend/**/*.test.ts'],
    environment: 'node',
    setupFiles: ['tests/frontend/setup.ts'],
  },
})
