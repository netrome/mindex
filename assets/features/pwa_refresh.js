const CACHE_PREFIX = "mindex-";
const RELOAD_DELAY_MS = 250;

const clearMindexCaches = async () => {
    if (!("caches" in window)) {
        return;
    }

    const cacheNames = await caches.keys();
    const deletions = cacheNames
        .filter((name) => name.startsWith(CACHE_PREFIX))
        .map((name) => caches.delete(name));
    await Promise.all(deletions);
};

const refreshPwa = async () => {
    if (!("serviceWorker" in navigator)) {
        window.location.reload();
        return;
    }

    const registration = await navigator.serviceWorker.getRegistration();
    if (!registration) {
        window.location.reload();
        return;
    }

    let didReload = false;
    const reloadOnce = () => {
        if (didReload) {
            return;
        }
        didReload = true;
        window.location.reload();
    };

    navigator.serviceWorker.addEventListener("controllerchange", reloadOnce, { once: true });

    try {
        await registration.update();
    } catch (err) {
        console.warn("PWA refresh: service worker update failed", err);
    }

    try {
        await clearMindexCaches();
    } catch (err) {
        console.warn("PWA refresh: cache clear failed", err);
    }

    if (registration.waiting) {
        registration.waiting.postMessage({ type: "SKIP_WAITING" });
    }

    setTimeout(reloadOnce, RELOAD_DELAY_MS);
};

export const initPwaRefresh = () => {
    const buttons = document.querySelectorAll("[data-pwa-refresh]");
    if (!buttons.length) {
        return;
    }

    buttons.forEach((button) => {
        button.addEventListener("click", (event) => {
            event.preventDefault();
            refreshPwa();
        });
    });
};
