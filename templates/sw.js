const AUTH_ENABLED = {{ auth_enabled }};
const CACHE_NAME = AUTH_ENABLED ? 'mindex-auth-v1' : 'mindex-v1';
const CACHE_PREFIX = 'mindex-';
const STATIC_ASSETS = [
  '/static/style.css',
  '/static/theme.js',
  '/static/app.js',
  '/static/mermaid.min.js',
  '/static/features/todo_toggle.js',
  '/static/features/reorder.js',
  '/static/features/push_subscribe.js',
  '/static/features/pwa_refresh.js',
  '/static/features/sw_register.js',
  '/static/manifest.json',
  '/sw.js'
];
if (!AUTH_ENABLED) {
  STATIC_ASSETS.unshift('/');
}

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

// Message event - allow manual refresh controls
self.addEventListener('message', event => {
  const data = event.data;
  if (!data || !data.type) {
    return;
  }

  if (data.type === 'SKIP_WAITING') {
    self.skipWaiting();
    return;
  }

  if (data.type === 'CLEAR_CACHES') {
    event.waitUntil(
      caches.keys()
        .then(cacheNames => {
          const deletions = cacheNames
            .filter(cacheName => cacheName.startsWith(CACHE_PREFIX))
            .map(cacheName => caches.delete(cacheName));
          return Promise.all(deletions);
        })
        .then(() => self.clients.claim())
    );
  }
});

// Fetch event - serve from cache when offline
self.addEventListener('fetch', event => {
  const { request } = event;
  const url = new URL(request.url);

  // Only handle same-origin requests
  if (url.origin !== location.origin) {
    return;
  }

  const isDocument = request.destination === 'document';
  const networkFirst = (isDocument || url.pathname.startsWith('/doc/')) && !AUTH_ENABLED;

  if (networkFirst) {
    event.respondWith(
      fetch(request)
        .then(response => {
          if (!AUTH_ENABLED && response && response.status === 200 && response.type === 'basic') {
            const responseToCache = response.clone();
            caches.open(CACHE_NAME)
              .then(cache => cache.put(request, responseToCache));
          }
          return response;
        })
        .catch(() => {
          return caches.match(request).then(cachedResponse => {
            if (cachedResponse) {
              return cachedResponse;
            }
            if (isDocument) {
              return new Response(
                '<html><body><h1>Offline</h1><p>This page is not available offline.</p></body></html>',
                { headers: { 'Content-Type': 'text/html' } }
              );
            }
          });
        })
    );
    return;
  }

  if (AUTH_ENABLED) {
    if (shouldCache(url.pathname)) {
      event.respondWith(
        caches.match(request)
          .then(cachedResponse => {
            if (cachedResponse) {
              return cachedResponse;
            }
            return fetch(request).then(response => {
              if (response && response.status === 200 && response.type === 'basic') {
                const responseToCache = response.clone();
                caches.open(CACHE_NAME)
                  .then(cache => cache.put(request, responseToCache));
              }
              return response;
            });
          })
      );
      return;
    }

    event.respondWith(fetch(request));
    return;
  }

  event.respondWith(
    caches.match(request)
      .then(cachedResponse => {
        if (cachedResponse) {
          return cachedResponse;
        }

        return fetch(request)
          .then(response => {
            if (!response || response.status !== 200 || response.type !== 'basic') {
              return response;
            }

            const responseToCache = response.clone();

            if (shouldCache(url.pathname)) {
              caches.open(CACHE_NAME)
                .then(cache => cache.put(request, responseToCache));
            }

            return response;
          })
          .catch(() => {
            if (isDocument) {
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
  if (AUTH_ENABLED) {
    return pathname === '/sw.js' || pathname.startsWith('/static/');
  }

  // Cache static assets
  if (pathname === '/sw.js' || pathname.startsWith('/static/')) {
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
