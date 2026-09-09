// Keep deployed cache names and worker URL to reuse existing model downloads.
const ASSET_CACHE = 'openmmo-models-v1'
const MANIFEST_CACHE = 'openmmo-model-manifests-v1'
const MANIFEST_KEY = '/__openmmo_model_manifest__'
const CACHE_INDEX_KEY = '/__openmmo_asset_cache_index__'
const MAX_CACHE_BYTES = 500_000_000
const MANIFEST_MESSAGE = 'openmmo:asset-manifest'
const OWNED_CACHE_PREFIX = 'openmmo-model'
const HASHED_ASSET_URL =
  /^\/(?:(?:models|textures)\/.+\.[0-9a-f]{8}\.glb|bgm\/.+\.[0-9a-f]{8}\.(?:mp3|m4a|ogg))$/i
const inFlight = new Map()
let cacheUpdates = Promise.resolve()
let cacheIndex

function updateCache(operation) {
  cacheUpdates = cacheUpdates.then(operation).catch(() => {})
  return cacheUpdates
}

self.addEventListener('install', () => self.skipWaiting())

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((names) =>
        Promise.all(
          names
            .filter(
              (name) =>
                name.startsWith(OWNED_CACHE_PREFIX) &&
                name !== ASSET_CACHE &&
                name !== MANIFEST_CACHE
            )
            .map((name) => caches.delete(name))
        )
      )
      .then(() =>
        updateCache(async () => {
          const cache = await caches.open(ASSET_CACHE)
          await loadCacheIndex(cache)
          await trimCache(cache, MAX_CACHE_BYTES)
          await saveCacheIndex()
        })
      )
      .then(() => self.clients.claim())
  )
})

self.addEventListener('fetch', (event) => {
  const request = event.request
  const url = new URL(request.url)

  if (
    request.method !== 'GET' ||
    url.origin !== self.location.origin ||
    !HASHED_ASSET_URL.test(url.pathname)
  ) {
    return
  }

  const operation = resolveAsset(request)
  event.respondWith(operation.then(({ response }) => response))
  event.waitUntil(operation.then(({ cacheWrite }) => cacheWrite))
})

self.addEventListener('message', (event) => {
  if (event.data?.type !== MANIFEST_MESSAGE) return
  event.waitUntil(updateCache(() => updateManifest(event.data.urls)))
})

async function resolveAsset(request) {
  let cache
  let hasCachedFile = false
  try {
    cache = await caches.open(ASSET_CACHE)
    const cached = await cache.match(request)
    if (cached) {
      hasCachedFile = true
      const response = request.headers.has('range')
        ? await readRange(request, cached)
        : cached
      if (response) {
        const usedAt = Date.now()
        const cacheWrite = updateCache(async () => {
          await loadCacheIndex(cache)
          const entry = cacheIndex.get(request.url)
          if (entry) entry.lastUsed = Math.max(entry.lastUsed, usedAt)
          await trimCache(cache, MAX_CACHE_BYTES)
          await saveCacheIndex()
        })
        return { response, cacheWrite }
      }
    }
  } catch {
    cache = null
  }

  if (request.headers.has('range')) {
    const streaming = fetch(request)
    let cacheWrite = Promise.resolve()
    if (cache && !hasCachedFile) {
      const headers = new Headers(request.headers)
      for (const name of [
        'range',
        'if-range',
        'if-none-match',
        'if-modified-since',
        'if-match',
        'if-unmodified-since',
      ]) {
        headers.delete(name)
      }
      const fullRequest = new Request(request, { headers, signal: null })
      cacheWrite = downloadAsset(fullRequest, cache)
        .then(({ cacheWrite }) => cacheWrite)
        .catch(() => {})
    }
    return { response: await streaming, cacheWrite }
  }

  const { response, cacheWrite } = await downloadAsset(request, cache)
  return { response: response.clone(), cacheWrite }
}

function downloadAsset(request, cache) {
  let network = inFlight.get(request.url)
  if (!network) {
    network = fetchAndCache(request, cache)
    inFlight.set(request.url, network)
    void network
      .then(({ cacheWrite }) => cacheWrite)
      .catch(() => {})
      .finally(() => inFlight.delete(request.url))
  }

  return network
}

async function readRange(request, cached) {
  if (request.headers.has('if-range')) return null
  const match = /^bytes=(\d*)-(\d*)$/i.exec(request.headers.get('range'))
  if (!match || (!match[1] && !match[2])) return null
  const blob = await cached.blob()
  const start = match[1]
    ? Number(match[1])
    : Math.max(0, blob.size - Number(match[2]))
  const end = match[1] && match[2] ? Number(match[2]) : blob.size - 1
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end)) return null
  if (start >= blob.size || end < start) {
    return new Response(null, {
      status: 416,
      headers: { 'Content-Range': `bytes */${blob.size}` },
    })
  }
  const last = Math.min(end, blob.size - 1)
  const headers = new Headers(cached.headers)
  headers.delete('Content-Encoding')
  headers.set('Content-Range', `bytes ${start}-${last}/${blob.size}`)
  headers.set('Content-Length', String(last - start + 1))
  headers.set('Accept-Ranges', 'bytes')
  return new Response(blob.slice(start, last + 1), { status: 206, headers })
}

async function fetchAndCache(request, cache) {
  const response = await fetch(request)
  let cacheWrite = Promise.resolve()
  if (cache && response.status === 200 && response.type === 'basic') {
    cacheWrite = storeAsset(cache, request, response.clone()).catch(() => {})
  }
  return { response, cacheWrite }
}

async function loadCacheIndex(cache) {
  if (cacheIndex) return
  let saved
  try {
    const metadata = await caches.open(MANIFEST_CACHE)
    saved = new Map(await (await metadata.match(CACHE_INDEX_KEY)).json())
  } catch {
    saved = new Map()
  }
  const entries = new Map()
  for (const request of await cache.keys()) {
    const entry = saved.get(request.url)
    if (
      Number.isSafeInteger(entry?.size) &&
      entry.size >= 0 &&
      Number.isFinite(entry?.lastUsed) &&
      entry.lastUsed >= 0
    ) {
      entries.set(request.url, entry)
    } else {
      const response = await cache.match(request)
      if (response) {
        entries.set(request.url, {
          size: (await response.blob()).size,
          lastUsed: 0,
        })
      }
    }
  }
  cacheIndex = entries
}

async function saveCacheIndex() {
  const metadata = await caches.open(MANIFEST_CACHE)
  await metadata.put(
    CACHE_INDEX_KEY,
    new Response(JSON.stringify([...cacheIndex]))
  )
}

async function trimCache(cache, limit) {
  let total = [...cacheIndex.values()].reduce(
    (sum, entry) => sum + entry.size,
    0
  )
  if (total <= limit) return
  const oldest = [...cacheIndex].sort((a, b) => a[1].lastUsed - b[1].lastUsed)
  for (const [url, entry] of oldest) {
    if (total <= limit) break
    await cache.delete(url)
    cacheIndex.delete(url)
    total -= entry.size
  }
}

async function storeAsset(cache, request, response) {
  const size = (await response.clone().blob()).size
  if (size > MAX_CACHE_BYTES) return
  const usedAt = Date.now()
  await updateCache(async () => {
    await loadCacheIndex(cache)
    const previous = cacheIndex.get(request.url)
    if (previous && (await cache.match(request))) {
      previous.lastUsed = Math.max(previous.lastUsed, usedAt)
    } else {
      cacheIndex.delete(request.url)
      await trimCache(cache, MAX_CACHE_BYTES - size)
      try {
        await cache.put(request, response)
      } catch (error) {
        await saveCacheIndex()
        throw error
      }
      cacheIndex.set(request.url, { size, lastUsed: usedAt })
    }
    await saveCacheIndex()
  })
}

async function updateManifest(rawUrls) {
  const urls = normalizeManifest(rawUrls)
  if (urls.length === 0) return

  const manifestCache = await caches.open(MANIFEST_CACHE)
  const previousState = await readManifestState(manifestCache)
  const known =
    sameManifest(urls, previousState.current) ||
    sameManifest(urls, previousState.previous)
  const state = known
    ? previousState
    : { current: urls, previous: previousState.current }

  if (!known) {
    await manifestCache.put(
      MANIFEST_KEY,
      new Response(JSON.stringify(state), {
        headers: { 'Content-Type': 'application/json' },
      })
    )
  }

  const keep = new Set([...state.current, ...state.previous])
  const assetCache = await caches.open(ASSET_CACHE)
  await loadCacheIndex(assetCache)
  const requests = await assetCache.keys()
  await Promise.all(
    requests
      .filter((request) => !keep.has(request.url))
      .map(async (request) => {
        await assetCache.delete(request)
        cacheIndex.delete(request.url)
      })
  )
  await trimCache(assetCache, MAX_CACHE_BYTES)
  await saveCacheIndex()
}

function normalizeManifest(rawUrls) {
  if (!Array.isArray(rawUrls)) return []

  const urls = rawUrls.flatMap((rawUrl) => {
    if (typeof rawUrl !== 'string') return []
    try {
      const url = new URL(rawUrl, self.location.origin)
      if (
        url.origin !== self.location.origin ||
        !HASHED_ASSET_URL.test(url.pathname)
      ) {
        return []
      }
      return [url.href]
    } catch {
      return []
    }
  })

  return [...new Set(urls)].sort()
}

async function readManifestState(cache) {
  try {
    const response = await cache.match(MANIFEST_KEY)
    if (!response) return { current: [], previous: [] }
    const state = await response.json()
    return {
      current: normalizeManifest(state.current),
      previous: normalizeManifest(state.previous),
    }
  } catch {
    return { current: [], previous: [] }
  }
}

function sameManifest(left, right) {
  return (
    left.length === right.length &&
    left.every((url, index) => url === right[index])
  )
}
