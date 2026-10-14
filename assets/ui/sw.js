// Earlier builds registered a cache-first service worker here, which could keep
// serving old JavaScript against a newer backend after an upgrade. Exospine no
// longer registers one. This file stays only so a profile that still has the
// old worker fetches this replacement on its next launch: it deletes every
// cache, unregisters itself and reloads the window from the app itself.
// Delete it once no supported release predates its removal.
self.addEventListener('install', () => self.skipWaiting());

self.addEventListener('activate', (e) => {
  e.waitUntil((async () => {
    for (const name of await caches.keys()) await caches.delete(name);
    await self.registration.unregister();
    for (const client of await self.clients.matchAll({ type: 'window' })) {
      client.navigate(client.url);
    }
  })());
});
