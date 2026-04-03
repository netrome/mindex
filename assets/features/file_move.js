const postMove = async (sourcePath, targetDir) => {
    const body = new URLSearchParams({
        source_path: sourcePath,
        target_dir: targetDir,
    });
    const response = await fetch("/api/d/move-file", {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body,
    });
    if (!response.ok) {
        const text = await response.text();
        throw new Error(text || "Failed to move file");
    }
};

export const initFileMove = () => {
    const page = document.querySelector(".move-page");
    if (!page) {
        return;
    }

    const notice = document.querySelector("[data-move-notice]");

    const setNotice = (message) => {
        if (notice) {
            notice.textContent = message;
        }
    };

    let dragState = null;
    let highlightedTarget = null;
    let autoScroll = null;

    const clearHighlight = () => {
        if (highlightedTarget) {
            highlightedTarget.classList.remove("move-drop-over");
        }
        highlightedTarget = null;
    };

    const stopAutoScroll = () => {
        if (autoScroll && autoScroll.raf) {
            cancelAnimationFrame(autoScroll.raf);
        }
        autoScroll = null;
    };

    const endDrag = () => {
        if (dragState && dragState.source) {
            dragState.source.classList.remove("is-dragging");
        }
        dragState = null;
        clearHighlight();
        stopAutoScroll();
    };

    const highlightTargetAt = (clientX, clientY) => {
        const el = document.elementFromPoint(clientX, clientY);
        if (!el) {
            clearHighlight();
            return null;
        }
        const target = el.closest(".move-drop-target");
        if (!target) {
            clearHighlight();
            return null;
        }
        if (highlightedTarget !== target) {
            clearHighlight();
            highlightedTarget = target;
            highlightedTarget.classList.add("move-drop-over");
        }
        return target;
    };

    const applyMove = async (sourcePath, targetDir) => {
        try {
            await postMove(sourcePath, targetDir);
            window.location.reload();
        } catch (err) {
            console.error(err);
            setNotice(err.message || "Failed to move file. Please try again.");
        }
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
                    highlightTargetAt(dragState.lastX, dragState.lastY);
                }
                autoScroll.raf = requestAnimationFrame(step);
            };
            autoScroll.raf = requestAnimationFrame(step);
        } else {
            autoScroll.speed = speed;
        }
    };

    // Native drag events (mouse)
    document.querySelectorAll(".move-handle").forEach((handle) => {
        handle.addEventListener("dragstart", (event) => {
            const source = handle.closest(".move-drag-source");
            if (!source) {
                return;
            }
            const filePath = source.dataset.filePath;
            if (!filePath) {
                return;
            }
            dragState = { type: "native", filePath, source };
            source.classList.add("is-dragging");
            if (event.dataTransfer) {
                event.dataTransfer.effectAllowed = "move";
                event.dataTransfer.setData("text/plain", filePath);
            }
        });

        handle.addEventListener("dragend", () => {
            endDrag();
        });

        // Pointer events (touch)
        handle.addEventListener("pointerdown", (event) => {
            if (event.pointerType === "mouse" || dragState) {
                return;
            }
            const source = handle.closest(".move-drag-source");
            if (!source) {
                return;
            }
            const filePath = source.dataset.filePath;
            if (!filePath) {
                return;
            }
            dragState = {
                type: "pointer",
                filePath,
                source,
                pointerId: event.pointerId,
                startX: event.clientX,
                startY: event.clientY,
                hasMoved: false,
                lastX: event.clientX,
                lastY: event.clientY,
            };
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
                dragState.source.classList.add("is-dragging");
            }
            dragState.lastX = event.clientX;
            dragState.lastY = event.clientY;
            highlightTargetAt(event.clientX, event.clientY);
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
            const target = highlightTargetAt(event.clientX, event.clientY);
            endDrag();
            if (!currentDrag.hasMoved || !target) {
                return;
            }
            const targetDir = target.dataset.dirPath;
            if (targetDir == null) {
                return;
            }
            await applyMove(currentDrag.filePath, targetDir);
            event.preventDefault();
        };

        handle.addEventListener("pointerup", finishPointerDrag);
        handle.addEventListener("pointercancel", finishPointerDrag);
    });

    // Drop target events (native drag)
    document.querySelectorAll(".move-drop-target").forEach((target) => {
        target.addEventListener("dragover", (event) => {
            if (!dragState || dragState.type === "pointer") {
                return;
            }
            event.preventDefault();
            if (highlightedTarget !== target) {
                clearHighlight();
                highlightedTarget = target;
                highlightedTarget.classList.add("move-drop-over");
            }
        });

        target.addEventListener("dragleave", () => {
            if (highlightedTarget === target) {
                clearHighlight();
            }
        });

        target.addEventListener("drop", async (event) => {
            event.preventDefault();
            if (!dragState || dragState.type === "pointer") {
                return;
            }
            const currentDrag = dragState;
            endDrag();
            const targetDir = target.dataset.dirPath;
            if (targetDir == null) {
                return;
            }
            await applyMove(currentDrag.filePath, targetDir);
        });
    });
};
