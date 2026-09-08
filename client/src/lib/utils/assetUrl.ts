import { DefaultLoadingManager } from 'three'

// Original path -> content-hashed path, inlined into index.html by
// scripts/hash-assets.mjs. Absent in dev, so paths pass through unchanged.
declare global {
  interface Window {
    __ASSET_MANIFEST__?: Record<string, string>
  }
}

const manifest = new Map(
  Object.entries(
    (typeof window !== 'undefined' && window.__ASSET_MANIFEST__) || {}
  )
)

const HASHED_MODEL_URL = /^\/models\/.+\.[0-9a-f]{8}\.glb$/i

export function assetUrl(path: string): string {
  return manifest.get(path) ?? path
}

export function getHashedModelAssetUrls(): string[] {
  return [...new Set(manifest.values())]
    .filter((url) => HASHED_MODEL_URL.test(url))
    .sort()
}

if (manifest.size > 0) DefaultLoadingManager.setURLModifier(assetUrl)
