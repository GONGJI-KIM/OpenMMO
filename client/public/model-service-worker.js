const MODEL_CACHE = 'openmmo-models-v1'
const MANIFEST_CACHE = 'openmmo-model-manifests-v1'
const MANIFEST_KEY = '/__openmmo_model_manifest__'
const MANIFEST_MESSAGE = 'openmmo:model-manifest'
const OWNED_CACHE_PREFIX = 'openmmo-model'
const HASHED_MODEL_URL = /^\/models\/.+\.[0-9a-f]{8}\.glb$/i
const inFlight = new Map()

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
                name !== MODEL_CACHE &&
                name !== MANIFEST_CACHE
            )
            .map((name) => caches.delete(name))
        )
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
    !HASHED_MODEL_URL.test(url.pathname) ||
    request.headers.has('range')
  ) {
    return
  }

  const operation = resolveModel(request)
  event.respondWith(operation.then(({ response }) => response))
  event.waitUntil(operation.then(({ cacheWrite }) => cacheWrite))
})

self.addEventListener('message', (event) => {
  if (event.data?.type !== MANIFEST_MESSAGE) return
  event.waitUntil(updateManifest(event.data.urls))
})

async function resolveModel(request) {
  let cache
  try {
    cache = await caches.open(MODEL_CACHE)
    const cached = await cache.match(request)
    if (cached) {
      return { response: cached, cacheWrite: Promise.resolve() }
    }
  } catch {
    cache = null
  }

  let network = inFlight.get(request.url)
  if (!network) {
    network = fetchAndCache(request, cache)
    inFlight.set(request.url, network)
    void network
      .then(({ cacheWrite }) => cacheWrite)
      .catch(() => {})
      .finally(() => inFlight.delete(request.url))
  }

  const { response, cacheWrite } = await network
  return { response: response.clone(), cacheWrite }
}

async function fetchAndCache(request, cache) {
  const response = await fetch(request)
  let cacheWrite = Promise.resolve()
  if (cache && response.status === 200 && response.type === 'basic') {
    cacheWrite = cache.put(request, response.clone()).catch(() => {})
  }
  return { response, cacheWrite }
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
  const modelCache = await caches.open(MODEL_CACHE)
  const requests = await modelCache.keys()
  await Promise.all(
    requests
      .filter((request) => !keep.has(request.url))
      .map((request) => modelCache.delete(request))
  )
}

function normalizeManifest(rawUrls) {
  if (!Array.isArray(rawUrls)) return []

  const urls = rawUrls.flatMap((rawUrl) => {
    if (typeof rawUrl !== 'string') return []
    try {
      const url = new URL(rawUrl, self.location.origin)
      if (
        url.origin !== self.location.origin ||
        !HASHED_MODEL_URL.test(url.pathname)
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
