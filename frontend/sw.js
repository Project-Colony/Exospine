// Exospine Service Worker — cache-first for static assets
const CACHE_NAME = 'exospine-v2';

self.addEventListener('install', (e) => {
  e.waitUntil(
    caches.open(CACHE_NAME).then((cache) =>
      cache.addAll([
        '/', '/index.html', '/styles/main.css',
        '/js/app.js', '/js/api.js', '/js/i18n.js', '/js/date_format.js',
        '/js/plugin_api.js',
        '/js/views/sidebar.js', '/js/views/mail_list.js', '/js/views/mail_view.js',
        '/js/views/compose.js', '/js/views/settings.js', '/js/views/onboarding.js',
        '/js/views/analytics.js', '/js/views/calendar.js', '/js/views/contacts.js',
        '/js/components/toast.js', '/js/components/dialog.js', '/js/components/context_menu.js',
      ])
    )
  );
  self.skipWaiting();
});

self.addEventListener('activate', (e) => {
  e.waitUntil(
    caches.keys().then((names) =>
      Promise.all(names.filter((n) => n !== CACHE_NAME).map((n) => caches.delete(n)))
    )
  );
  self.clients.claim();
});

self.addEventListener('fetch', (e) => {
  // Only cache GET requests for static assets
  if (e.request.method !== 'GET') return;
  const url = new URL(e.request.url);
  if (url.protocol === 'ipc:' || url.hostname === 'ipc.localhost') return;

  e.respondWith(
    caches.match(e.request).then((cached) => {
      if (cached) return cached;
      return fetch(e.request).then((response) => {
        if (response.ok && url.origin === location.origin) {
          const clone = response.clone();
          caches.open(CACHE_NAME).then((c) => c.put(e.request, clone));
        }
        return response;
      });
    }).catch(() => caches.match('/index.html'))
  );
});
