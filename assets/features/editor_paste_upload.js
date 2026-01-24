const initEditorPasteUploads = () => {
    const textarea = document.querySelector("[data-editor]");
    const status = document.querySelector("[data-editor-upload-status]");

    if (!textarea) {
        return;
    }

    const setStatus = (message, kind = "info") => {
        if (!status) {
            return;
        }
        status.textContent = message;
        status.dataset.state = kind;
    };

    const insertAtCursor = (text) => {
        const start = textarea.selectionStart || 0;
        const end = textarea.selectionEnd || 0;
        const before = textarea.value.slice(0, start);
        const after = textarea.value.slice(end);
        textarea.value = `${before}${text}${after}`;
        const next = start + text.length;
        textarea.selectionStart = next;
        textarea.selectionEnd = next;
        textarea.focus();
    };

    const findImageItem = (items) => {
        if (!items) {
            return null;
        }
        for (const item of items) {
            if (item.kind === "file" && item.type && item.type.startsWith("image/")) {
                return item;
            }
        }
        return null;
    };

    textarea.addEventListener("paste", async (event) => {
        const item = findImageItem(event.clipboardData?.items || []);
        if (!item) {
            return;
        }

        event.preventDefault();
        const file = item.getAsFile();
        if (!file) {
            setStatus("Clipboard image unavailable.", "error");
            return;
        }

        setStatus("Uploading image...");

        try {
            const response = await fetch("/api/uploads", {
                method: "POST",
                headers: {
                    "Content-Type": file.type || "application/octet-stream",
                    "X-Upload-Filename": file.name || "paste",
                },
                body: file,
            });

            const payload = await response.json().catch(() => null);
            if (!response.ok) {
                const message = payload && payload.error ? payload.error : "Upload failed.";
                setStatus(message, "error");
                return;
            }

            const markdown = payload && payload.markdown ? payload.markdown : "";
            if (markdown) {
                insertAtCursor(markdown);
                setStatus("Image uploaded.");
            } else {
                setStatus("Upload succeeded, but no markdown returned.", "error");
            }
        } catch (err) {
            setStatus("Upload failed.", "error");
        }
    });
};

export { initEditorPasteUploads };
