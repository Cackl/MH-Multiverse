import { describe, it, expect } from 'vitest'
import { versionCaps } from '../../src/lib/versionCaps'

describe('versionCaps', () => {
  it.each([
    // version, custom, overlay, steamMode, infinity, patchLabels, nightly
    ['1.48', false, false, false, false, false, true],
    ['1.52', false, true, true, true, true, true],
    ['1.53', false, false, false, true, false, true],
    ['', false, true, true, true, true, false],
    // Custom only turns off nightlies. The rest follows the base version.
    ['1.48', true, false, false, false, false, false],
    ['1.52', true, true, true, true, true, false],
    ['1.53', true, false, false, true, false, false],
  ] as const)('%s custom=%s', (version, custom, overlay, steamMode, infinity, patchLabels, nightly) => {
    expect(versionCaps(version, custom)).toEqual({ overlay, steamMode, infinity, patchLabels, nightly })
  })
})
