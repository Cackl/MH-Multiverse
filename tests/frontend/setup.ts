// Stub @tauri-apps/api/core so modules that import `invoke` don't crash
// during unit tests. Individual tests can override with vi.mock().
import { vi } from 'vitest'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockRejectedValue(new Error('invoke not mocked for this test')),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}))

// Node's built-in `localStorage` global throws SecurityError on first touch
// (even a bare `typeof localStorage`) unless started with --localstorage-file.
// Replace it with a working in-memory stub rather than pulling in jsdom just
// for this.
const memoryStorage = new Map<string, string>()
vi.stubGlobal('localStorage', {
  getItem: (key: string) => memoryStorage.get(key) ?? null,
  setItem: (key: string, value: string) => memoryStorage.set(key, value),
  removeItem: (key: string) => memoryStorage.delete(key),
  clear: () => memoryStorage.clear(),
})
