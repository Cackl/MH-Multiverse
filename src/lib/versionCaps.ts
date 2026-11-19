/** What the app can offer for a linked version. '' is a version that wasn't detected or picked,
 *  which keeps today's 1.52 behaviour except for nightly updates. */
export interface VersionCaps {
  /** The overlay hook is built for the 1.52 client. */
  overlay: boolean
  /** 1.48 and 1.53 always launch with -robocopy -nosteam (provisional). */
  steamMode: boolean
  /** 1.48 has no Infinity system or difficulty choice, so Omega XP is the live one. */
  infinity: boolean
  /** The Patches tab's names and descriptions for stock patch files describe 1.52's files. */
  patchLabels: boolean
  /** Off for Custom servers and for servers whose version isn't known. */
  nightly: boolean
}

export function versionCaps(version: string, custom: boolean): VersionCaps {
  const notPc152 = version === '1.48' || version === '1.53'
  return {
    overlay: !notPc152,
    steamMode: !notPc152,
    infinity: version !== '1.48',
    patchLabels: !notPc152,
    nightly: !custom && ['1.48', '1.52', '1.53'].includes(version),
  }
}

export type VersionMatch = 'match' | 'mismatch' | 'unknown'

/** Whether a detected client and server version can play together. null is not detected. */
export function versionMatch(client: string | null, server: string | null): VersionMatch {
  if (!client || !server) return 'unknown'
  return client === server ? 'match' : 'mismatch'
}
