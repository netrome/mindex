import { filter } from "./fuzzy.js";

// fzf-style command palette. Opened with Ctrl/Cmd-K from any page; a first
// keystroke chooses a mode (`f` = fuzzy file open). Results are keyboard- and
// click-navigable. The overlay is built once and reused.

const MAX_RESULTS = 50;

export const initPalette = () => {
    if (document.querySelector(".palette-overlay")) {
        return;
    }

    const overlay = document.createElement("div");
    overlay.className = "palette-overlay";
    overlay.hidden = true;

    const panel = document.createElement("div");
    panel.className = "palette";
    panel.setAttribute("role", "dialog");
    panel.setAttribute("aria-modal", "true");
    panel.setAttribute("aria-label", "Command palette");

    const input = document.createElement("input");
    input.type = "text";
    input.className = "palette-input";
    input.autocomplete = "off";
    input.spellcheck = false;
    input.setAttribute("aria-label", "Command palette");

    const menu = document.createElement("ul");
    menu.className = "palette-menu";

    const results = document.createElement("ul");
    results.className = "palette-results";
    results.setAttribute("role", "listbox");
    results.hidden = true;

    panel.append(input, menu, results);
    overlay.append(panel);
    document.body.append(overlay);

    let mode = null; // null = mode menu, "file" = fuzzy file open
    let entries = []; // currently rendered file entries
    let activeIndex = -1;
    let fileCache = null; // [{ path, kind }], fetched once per page load

    const isTextEntry = (el) =>
        !!el &&
        (el.tagName === "INPUT" ||
            el.tagName === "TEXTAREA" ||
            el.isContentEditable);

    const navigate = (entry) => {
        const encoded = entry.path
            .split("/")
            .map(encodeURIComponent)
            .join("/");
        // `/d/<path>` resolves to the correct view for any file kind.
        window.location.href = `/d/${encoded}`;
    };

    const setActive = (index) => {
        if (entries.length === 0) {
            activeIndex = -1;
            return;
        }
        activeIndex = Math.max(0, Math.min(index, entries.length - 1));
        const rows = results.children;
        for (let i = 0; i < rows.length; i++) {
            const isActive = i === activeIndex;
            rows[i].classList.toggle("is-active", isActive);
            rows[i].setAttribute("aria-selected", isActive ? "true" : "false");
            if (isActive) {
                rows[i].scrollIntoView({ block: "nearest" });
            }
        }
    };

    const showMessage = (text) => {
        const item = document.createElement("li");
        item.className = "palette-empty";
        item.textContent = text;
        results.replaceChildren(item);
    };

    const renderResults = () => {
        if (entries.length === 0) {
            showMessage("No matching files.");
            activeIndex = -1;
            return;
        }
        const rows = entries.map((entry, index) => {
            const item = document.createElement("li");
            item.className = "palette-result";
            item.setAttribute("role", "option");

            const path = document.createElement("span");
            path.className = "palette-result-path";
            path.textContent = entry.path;

            const kind = document.createElement("span");
            kind.className = "palette-result-kind";
            kind.textContent = entry.kind;

            item.append(path, kind);
            item.addEventListener("mousedown", (event) => {
                event.preventDefault();
                navigate(entry);
            });
            item.addEventListener("mousemove", () => setActive(index));
            return item;
        });
        results.replaceChildren(...rows);
        setActive(0);
    };

    const updateResults = () => {
        if (!fileCache) {
            return;
        }
        entries = filter(input.value, fileCache, MAX_RESULTS);
        renderResults();
    };

    const loadFiles = async () => {
        if (fileCache) {
            return;
        }
        try {
            const response = await fetch("/api/files", {
                headers: { Accept: "application/json" },
            });
            if (!response.ok) {
                throw new Error(`status ${response.status}`);
            }
            fileCache = await response.json();
        } catch (err) {
            console.error("palette: failed to load file list", err);
            fileCache = [];
            showMessage("Could not load files.");
        }
    };

    const enterFileMode = async () => {
        mode = "file";
        input.value = "";
        input.placeholder = "Open file…";
        menu.hidden = true;
        results.hidden = false;
        showMessage("Loading…");
        await loadFiles();
        if (mode === "file") {
            updateResults();
        }
    };

    // Modes keyed by their trigger character. (Content search arrives in a
    // later task; adding it here is a single entry.)
    const MODES = [{ key: "f", label: "Open file", enter: enterFileMode }];

    const renderMenu = () => {
        const items = MODES.map((def) => {
            const item = document.createElement("li");
            item.className = "palette-menu-item";

            const kbd = document.createElement("kbd");
            kbd.textContent = def.key;

            const label = document.createElement("span");
            label.textContent = def.label;

            item.append(kbd, label);
            item.addEventListener("mousedown", (event) => {
                event.preventDefault(); // keep focus in the input
                def.enter();
            });
            return item;
        });
        menu.replaceChildren(...items);
    };

    const resetToMenu = () => {
        mode = null;
        entries = [];
        activeIndex = -1;
        input.value = "";
        input.placeholder = "Select a mode…";
        menu.hidden = false;
        results.hidden = true;
        results.replaceChildren();
    };

    const open = () => {
        overlay.hidden = false;
        resetToMenu();
        input.focus();
    };

    const close = () => {
        overlay.hidden = true;
        input.value = "";
    };

    input.addEventListener("keydown", (event) => {
        if (mode === null) {
            if (event.ctrlKey || event.metaKey || event.altKey) {
                return;
            }
            if (event.key.length === 1) {
                // Keep the menu field empty; only mode keys do anything.
                event.preventDefault();
                const selected = MODES.find((m) => m.key === event.key);
                if (selected) {
                    selected.enter();
                }
            }
            return;
        }
        if (event.key === "ArrowDown") {
            event.preventDefault();
            setActive(activeIndex + 1);
        } else if (event.key === "ArrowUp") {
            event.preventDefault();
            setActive(activeIndex - 1);
        } else if (event.key === "Enter") {
            event.preventDefault();
            if (activeIndex >= 0 && entries[activeIndex]) {
                navigate(entries[activeIndex]);
            }
        }
    });

    input.addEventListener("input", () => {
        if (mode === "file") {
            updateResults();
        }
    });

    overlay.addEventListener("mousedown", (event) => {
        if (event.target === overlay) {
            close();
        }
    });

    document.addEventListener("keydown", (event) => {
        const key = event.key.toLowerCase();
        if (
            (event.metaKey || event.ctrlKey) &&
            !event.altKey &&
            !event.shiftKey &&
            key === "k"
        ) {
            if (!overlay.hidden) {
                event.preventDefault();
                return;
            }
            if (isTextEntry(document.activeElement)) {
                return; // don't steal Ctrl/Cmd-K while the user is typing
            }
            event.preventDefault();
            open();
            return;
        }
        if (event.key === "Escape" && !overlay.hidden) {
            event.preventDefault();
            close();
        }
    });

    renderMenu();
};
