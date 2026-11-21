import { describe, it, expect } from 'vitest'
import { versionCaps, versionMatch } from '../../src/lib/versionCaps'

describe('versionMatch', () => {
  it.each([
    ['1.48', '1.48', 'match'],
    ['1.52', '1.48', 'mismatch'],
    ['1.53', '1.52', 'mismatch'],
    [null, '1.52', 'unknown'],
    ['1.52', null, 'unknown'],
    [null, null, 'unknown'],
  ] as const)('client %s server %s', (client, server, expected) => {
    expect(versionMatch(client, server)).toBe(expected)
  })
})

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
