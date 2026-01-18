const getActiveMode = (page) => {
    const mode = page.dataset.mode;
    return mode === "line" ? "line" : "block";
};

const setActiveMode = (page, mode) => {
    page.dataset.mode = mode;
    document.querySelectorAll(".reorder-mode-toggle").forEach((button) => {
        const isActive = button.dataset.mode === mode;
        button.setAttribute("aria-pressed", isActive ? "true" : "false");
    });
    const url = new URL(window.location.href);
    url.searchParams.set("mode", mode);
    window.history.replaceState(null, "", url);
};

const postReorder = async (docId, payload) => {
    const body = new URLSearchParams({
        doc_id: docId,
        start_line: String(payload.startLine),
        end_line: String(payload.endLine),
        insert_before_line: String(payload.insertBeforeLine),
        mode: payload.mode,
    });
    const response = await fetch("/api/doc/reorder-range", {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body,
    });
    if (!response.ok) {
        const text = await response.text();
        throw new Error(text || "Failed to reorder");
    }
};

export const initReorder = () => {
    const page = document.querySelector(".reorder-page");
    if (!page) {
        return;
    }

    const notice = document.querySelector("[data-reorder-notice]");
    const docId = page.dataset.docId || "";
    const lineCount = Number.parseInt(page.dataset.lineCount, 10);
    if (!docId || !Number.isFinite(lineCount)) {
        return;
    }

    const setNotice = (message) => {
        if (notice) {
            notice.textContent = message;
        }
    };

    setActiveMode(page, getActiveMode(page));

    document.querySelectorAll(".reorder-mode-toggle").forEach((button) => {
        button.addEventListener("click", () => {
            const mode = button.dataset.mode === "line" ? "line" : "block";
            setActiveMode(page, mode);
        });
    });

    let dragState = null;
    let dropTarget = null;
    let dropPosition = null;

    const clearDropIndicator = () => {
        if (dropTarget) {
            dropTarget.classList.remove("drop-before", "drop-after");
        }
        dropTarget = null;
        dropPosition = null;
    };

    const getDropInfo = (event) => {
        const row = event.target.closest(".reorder-row");
        if (!row || !row.dataset.startLine || !row.dataset.endLine) {
            return { insertBeforeLine: lineCount, row: null, position: null };
        }
        const rect = row.getBoundingClientRect();
        const after = event.clientY > rect.top + rect.height / 2;
        const mode = getActiveMode(page);
        if (!after) {
            return {
                insertBeforeLine: Number.parseInt(row.dataset.startLine, 10),
                row,
                position: "before",
            };
        }

        if (mode === "line") {
            return {
                insertBeforeLine: Number.parseInt(row.dataset.endLine, 10) + 1,
                row,
                position: "after",
            };
        }

        const next = row.nextElementSibling;
        if (next && next.classList.contains("reorder-row")) {
            return {
                insertBeforeLine: Number.parseInt(next.dataset.startLine, 10),
                row,
                position: "after",
            };
        }
        return { insertBeforeLine: lineCount, row, position: "after" };
    };

    document.querySelectorAll(".reorder-row").forEach((row) => {
        row.addEventListener("dragstart", (event) => {
            const startLine = Number.parseInt(row.dataset.startLine, 10);
            const endLine = Number.parseInt(row.dataset.endLine, 10);
            if (!Number.isFinite(startLine) || !Number.isFinite(endLine)) {
                return;
            }
            dragState = { startLine, endLine, mode: getActiveMode(page) };
            row.classList.add("is-dragging");
            if (event.dataTransfer) {
                event.dataTransfer.effectAllowed = "move";
                event.dataTransfer.setData("text/plain", "reorder");
            }
        });

        row.addEventListener("dragend", () => {
            row.classList.remove("is-dragging");
            dragState = null;
            clearDropIndicator();
        });
    });

    document.querySelectorAll(".reorder-list").forEach((list) => {
        list.addEventListener("dragover", (event) => {
            event.preventDefault();
            if (!dragState) {
                return;
            }
            const info = getDropInfo(event);
            if (!info.row) {
                clearDropIndicator();
                return;
            }
            if (dropTarget !== info.row || dropPosition !== info.position) {
                clearDropIndicator();
                dropTarget = info.row;
                dropPosition = info.position;
                dropTarget.classList.add(
                    dropPosition === "after" ? "drop-after" : "drop-before"
                );
            }
        });

        list.addEventListener("dragleave", (event) => {
            if (!list.contains(event.relatedTarget)) {
                clearDropIndicator();
            }
        });

        list.addEventListener("drop", async (event) => {
            event.preventDefault();
            if (!dragState) {
                return;
            }
            const info = getDropInfo(event);
            clearDropIndicator();
            try {
                await postReorder(docId, {
                    startLine: dragState.startLine,
                    endLine: dragState.endLine,
                    insertBeforeLine: info.insertBeforeLine,
                    mode: dragState.mode,
                });
                const url = new URL(window.location.href);
                url.searchParams.set("mode", dragState.mode);
                window.location.assign(url);
            } catch (err) {
                console.error(err);
                setNotice("Failed to reorder. Please reload and try again.");
            }
        });
    });
};
