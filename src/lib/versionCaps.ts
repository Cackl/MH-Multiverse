/** What the app can offer for a linked version. '' is a version that wasn't detected or picked,
 *  which keeps today's 1.52 behaviour except for nightly updates. */
export interface VersionCaps {
  /** The overlay hook is built for the 1.52 client. */
  overlay: boolean
  /** 1.48 and 1.53 always launch with -robocopy -nosteam (provisional). */
  steamMode: boolean
  /** 1.48 has no Infinity system or difficulty choice, so Omega XP is the live one. */
  infinity: boolean
  /** The stock patch pack labels describe 1.52's files. */
  stockLabels: boolean
  /** Off for Custom servers and for servers whose version isn't known. */
  nightly: boolean
}

export function versionCaps(version: string, custom: boolean): VersionCaps {
  const notPc152 = version === '1.48' || version === '1.53'
  return {
    overlay: !notPc152,
    steamMode: !notPc152,
    infinity: version !== '1.48',
    stockLabels: !notPc152,
    nightly: !custom && ['1.48', '1.52', '1.53'].includes(version),
  }
}
