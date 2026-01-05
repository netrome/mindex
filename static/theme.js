(() => {
    const storageKey = "theme";
    const root = document.documentElement;
    const togglesSelector = "[data-theme-toggle]";
    const media = window.matchMedia("(prefers-color-scheme: dark)");

    const storedTheme = () => {
        const value = localStorage.getItem(storageKey);
        if (value === "light" || value === "dark") {
            return value;
        }
        return null;
    };

    const systemTheme = () => (media.matches ? "dark" : "light");

    const updateToggles = (theme) => {
        document.querySelectorAll(togglesSelector).forEach((button) => {
            const isDark = theme === "dark";
            button.setAttribute("aria-pressed", isDark ? "true" : "false");
            button.setAttribute(
                "title",
                isDark ? "Switch to light theme" : "Switch to dark theme"
            );
        });
    };

    const applyTheme = (theme) => {
        root.setAttribute("data-theme", theme);
        updateToggles(theme);
    };

    const init = () => {
        const stored = storedTheme();
        const initial = stored || systemTheme();
        applyTheme(initial);

        if (!stored) {
            media.addEventListener("change", (event) => {
                if (storedTheme()) {
                    return;
                }
                applyTheme(event.matches ? "dark" : "light");
            });
        }

        document.querySelectorAll(togglesSelector).forEach((button) => {
            button.addEventListener("click", () => {
                const current = root.getAttribute("data-theme") || systemTheme();
                const next = current === "dark" ? "light" : "dark";
                localStorage.setItem(storageKey, next);
                applyTheme(next);
            });
        });
    };

    if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", init);
    } else {
        init();
    }
})();
