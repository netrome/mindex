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
    let autoScroll = null;

    const clearDropIndicator = () => {
        if (dropTarget) {
            dropTarget.classList.remove("drop-before", "drop-after");
        }
        dropTarget = null;
        dropPosition = null;
    };

    const stopAutoScroll = () => {
        if (autoScroll && autoScroll.raf) {
            cancelAnimationFrame(autoScroll.raf);
        }
        autoScroll = null;
    };

    const endDrag = () => {
        if (dragState && dragState.row) {
            dragState.row.classList.remove("is-dragging");
        }
        dragState = null;
        clearDropIndicator();
        stopAutoScroll();
    };

    const buildDragState = (row, extra = {}) => {
        const startLine = Number.parseInt(row.dataset.startLine, 10);
        const endLine = Number.parseInt(row.dataset.endLine, 10);
        if (!Number.isFinite(startLine) || !Number.isFinite(endLine)) {
            return null;
        }
        return {
            startLine,
            endLine,
            mode: getActiveMode(page),
            row,
            ...extra,
        };
    };

    const isNoopMove = (drag, insertBeforeLine) =>
        insertBeforeLine >= drag.startLine &&
        insertBeforeLine <= drag.endLine + 1;

    const applyReorder = async (drag, insertBeforeLine) => {
        if (!drag || isNoopMove(drag, insertBeforeLine)) {
            return;
        }
        try {
            await postReorder(docId, {
                startLine: drag.startLine,
                endLine: drag.endLine,
                insertBeforeLine,
                mode: drag.mode,
            });
            const url = new URL(window.location.href);
            url.searchParams.set("mode", drag.mode);
            window.location.assign(url);
        } catch (err) {
            console.error(err);
            setNotice("Failed to reorder. Please reload and try again.");
        }
    };

    const getDropInfoForRow = (row, clientY) => {
        if (!row || !row.dataset.startLine || !row.dataset.endLine) {
            return { insertBeforeLine: lineCount, row: null, position: null };
        }
        const rect = row.getBoundingClientRect();
        const after = clientY > rect.top + rect.height / 2;
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

    const getDropInfoFromPoint = (clientX, clientY) => {
        const element = document.elementFromPoint(clientX, clientY);
        if (!element) {
            return {
                insertBeforeLine: lineCount,
                row: null,
                position: null,
                isOverList: false,
            };
        }
        const row = element.closest(".reorder-row");
        if (row) {
            return { ...getDropInfoForRow(row, clientY), isOverList: true };
        }
        const list = element.closest(".reorder-list");
        if (list) {
            return {
                insertBeforeLine: lineCount,
                row: null,
                position: null,
                isOverList: true,
            };
        }
        return {
            insertBeforeLine: lineCount,
            row: null,
            position: null,
            isOverList: false,
        };
    };

    const updateDropIndicatorAt = (clientX, clientY) => {
        const info = getDropInfoFromPoint(clientX, clientY);
        if (!info.row) {
            clearDropIndicator();
        } else if (dropTarget !== info.row || dropPosition !== info.position) {
            clearDropIndicator();
            dropTarget = info.row;
            dropPosition = info.position;
            dropTarget.classList.add(
                dropPosition === "after" ? "drop-after" : "drop-before"
            );
        }
        return info;
    };

    const updateAutoScroll = (clientY) => {
        if (!dragState || dragState.type !== "pointer" || !dragState.hasMoved) {
            stopAutoScroll();
            return;
        }
        const threshold = 48;
        const maxSpeed = 18;
        const height = window.innerHeight;
        let speed = 0;
        if (clientY < threshold) {
            speed = -((threshold - clientY) / threshold) * maxSpeed;
        } else if (clientY > height - threshold) {
            speed = ((clientY - (height - threshold)) / threshold) * maxSpeed;
        }
        if (speed === 0) {
            stopAutoScroll();
            return;
        }
        if (!autoScroll) {
            autoScroll = { speed, raf: 0 };
            const step = () => {
                if (!autoScroll || !dragState || dragState.type !== "pointer") {
                    stopAutoScroll();
                    return;
                }
                window.scrollBy(0, autoScroll.speed);
                if (
                    Number.isFinite(dragState.lastX) &&
                    Number.isFinite(dragState.lastY)
                ) {
                    updateDropIndicatorAt(dragState.lastX, dragState.lastY);
                }
                autoScroll.raf = requestAnimationFrame(step);
            };
            autoScroll.raf = requestAnimationFrame(step);
        } else {
            autoScroll.speed = speed;
        }
    };

    document.querySelectorAll(".reorder-row-handle").forEach((handle) => {
        handle.addEventListener("dragstart", (event) => {
            const row = handle.closest(".reorder-row");
            if (!row) {
                return;
            }
            const nextState = buildDragState(row);
            if (!nextState) {
                return;
            }
            dragState = { ...nextState, type: "native" };
            row.classList.add("is-dragging");
            if (event.dataTransfer) {
                event.dataTransfer.effectAllowed = "move";
                event.dataTransfer.setData("text/plain", "reorder");
            }
        });

        handle.addEventListener("dragend", () => {
            endDrag();
        });

        handle.addEventListener("pointerdown", (event) => {
            if (event.pointerType === "mouse" || dragState) {
                return;
            }
            const row = handle.closest(".reorder-row");
            if (!row) {
                return;
            }
            const nextState = buildDragState(row, {
                type: "pointer",
                pointerId: event.pointerId,
                startX: event.clientX,
                startY: event.clientY,
                hasMoved: false,
                lastX: event.clientX,
                lastY: event.clientY,
            });
            if (!nextState) {
                return;
            }
            dragState = nextState;
            if (handle.setPointerCapture) {
                handle.setPointerCapture(event.pointerId);
            }
            event.preventDefault();
        });

        handle.addEventListener("pointermove", (event) => {
            if (
                !dragState ||
                dragState.type !== "pointer" ||
                dragState.pointerId !== event.pointerId
            ) {
                return;
            }
            const dx = event.clientX - dragState.startX;
            const dy = event.clientY - dragState.startY;
            if (!dragState.hasMoved && Math.hypot(dx, dy) < 4) {
                return;
            }
            if (!dragState.hasMoved) {
                dragState.hasMoved = true;
                dragState.row.classList.add("is-dragging");
            }
            dragState.lastX = event.clientX;
            dragState.lastY = event.clientY;
            updateDropIndicatorAt(event.clientX, event.clientY);
            updateAutoScroll(event.clientY);
            event.preventDefault();
        });

        const finishPointerDrag = async (event) => {
            if (
                !dragState ||
                dragState.type !== "pointer" ||
                dragState.pointerId !== event.pointerId
            ) {
                return;
            }
            const currentDrag = dragState;
            const info = getDropInfoFromPoint(event.clientX, event.clientY);
            endDrag();
            if (!currentDrag.hasMoved || !info.isOverList) {
                return;
            }
            await applyReorder(currentDrag, info.insertBeforeLine);
            event.preventDefault();
        };

        handle.addEventListener("pointerup", finishPointerDrag);
        handle.addEventListener("pointercancel", finishPointerDrag);
    });

    document.querySelectorAll(".reorder-list").forEach((list) => {
        list.addEventListener("dragover", (event) => {
            event.preventDefault();
            if (!dragState || dragState.type === "pointer") {
                return;
            }
            const row = event.target.closest(".reorder-row");
            const info = getDropInfoForRow(row, event.clientY);
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
            if (!dragState || dragState.type === "pointer") {
                return;
            }
            const currentDrag = dragState;
            const row = event.target.closest(".reorder-row");
            const info = getDropInfoForRow(row, event.clientY);
            endDrag();
            await applyReorder(currentDrag, info.insertBeforeLine);
        });
    });
};
