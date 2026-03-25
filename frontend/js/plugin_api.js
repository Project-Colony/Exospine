// Exospine Plugin API
// Allows third-party extensions to hook into the email client.

const _plugins = [];

export function registerPlugin(plugin) {
  if (!plugin.name || !plugin.version) {
    throw new Error('Plugin must have name and version');
  }
  _plugins.push(plugin);
  console.info(`Plugin loaded: ${plugin.name} v${plugin.version}`);
  if (typeof plugin.onInit === 'function') {
    try { plugin.onInit(getPluginContext()); } catch (e) {
      console.error(`Plugin ${plugin.name} init failed:`, e);
    }
  }
}

export function getPlugins() { return [..._plugins]; }

function getPluginContext() {
  return {
    showToast: (msg, level) => {
      const { showToast } = await_import('./components/toast.js');
      if (showToast) showToast(msg, level);
    },
    getVersion: () => '0.2.0',
    getTheme: () => document.documentElement.getAttribute('data-theme') || 'dark',
  };
}

// Lazy import helper (plugins can't use ES modules easily)
async function await_import(path) {
  try { return await import(path); } catch { return {}; }
}
