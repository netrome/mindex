const CACHE_NAME = 'mindex-v1';
const STATIC_ASSETS = [
  '/',
  '/static/style.css',
  '/static/theme.js',
  '/static/manifest.json',
  '/sw.js'
];

// Install event - cache static assets
self.addEventListener('install', event => {
  event.waitUntil(
    caches.open(CACHE_NAME)
      .then(cache => cache.addAll(STATIC_ASSETS))
      .then(() => self.skipWaiting())
  );
});

// Activate event - clean up old caches
self.addEventListener('activate', event => {
  event.waitUntil(
    caches.keys()
      .then(cacheNames => {
        return Promise.all(
          cacheNames.map(cacheName => {
            if (cacheName !== CACHE_NAME) {
              return caches.delete(cacheName);
            }
          })
        );
      })
      .then(() => self.clients.claim())
  );
});

// Fetch event - serve from cache when offline
self.addEventListener('fetch', event => {
  const { request } = event;
  const url = new URL(request.url);

  // Only handle same-origin requests
  if (url.origin !== location.origin) {
    return;
  }

  event.respondWith(
    caches.match(request)
      .then(cachedResponse => {
        // If we have a cached response, serve it
        if (cachedResponse) {
          return cachedResponse;
        }

        // Otherwise, fetch from network and cache the response
        return fetch(request)
          .then(response => {
            // Don't cache non-successful responses
            if (!response || response.status !== 200 || response.type !== 'basic') {
              return response;
            }

            // Cache documents and static assets for offline viewing
            const responseToCache = response.clone();

            if (shouldCache(url.pathname)) {
              caches.open(CACHE_NAME)
                .then(cache => cache.put(request, responseToCache));
            }

            return response;
          })
          .catch(() => {
            // If network fails and we don't have cache, return a basic offline page
            if (request.destination === 'document') {
              return new Response(
                '<html><body><h1>Offline</h1><p>This page is not available offline.</p></body></html>',
                { headers: { 'Content-Type': 'text/html' } }
              );
            }
          });
      })
  );
});

// Push event - display notification
self.addEventListener('push', event => {
  let title = 'Mindex';
  let body = 'You have a notification.';

  if (event.data) {
    try {
      const data = event.data.json();
      if (data && (data.title || data.body)) {
        title = data.title || title;
        body = data.body || body;
      } else {
        body = event.data.text();
      }
    } catch (err) {
      body = event.data.text();
    }
  }

  event.waitUntil(
    self.registration.showNotification(title, {
      body,
      icon: '/static/icons/icon-192.png',
      badge: '/static/icons/icon-192.png'
    })
  );
});

self.addEventListener('notificationclick', event => {
  event.notification.close();
  event.waitUntil(self.clients.openWindow('/'));
});

// Determine if a URL should be cached
function shouldCache(pathname) {
  // Cache static assets
  if (pathname.startsWith('/static/')) {
    return true;
  }

  // Cache document views (but not edits)
  if (pathname.startsWith('/doc/') || pathname === '/') {
    return true;
  }

  // Cache search results
  if (pathname === '/search') {
    return true;
  }

  return false;
}
