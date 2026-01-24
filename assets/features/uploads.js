const initUploads = () => {
    const root = document.querySelector("[data-upload]");
    if (!root) {
        return;
    }

    const input = root.querySelector("[data-upload-input]");
    const button = root.querySelector("[data-upload-button]");
    const status = root.querySelector("[data-upload-status]");
    const output = root.querySelector("[data-upload-output]");
    const urlField = root.querySelector("[data-upload-url]");
    const markdownField = root.querySelector("[data-upload-markdown]");
    const copyUrl = root.querySelector("[data-copy-url]");
    const copyMarkdown = root.querySelector("[data-copy-markdown]");

    if (!input || !button || !status || !output || !urlField || !markdownField) {
        return;
    }

    const setStatus = (message, kind = "info") => {
        status.textContent = message;
        status.dataset.state = kind;
    };

    const copyText = async (text) => {
        if (navigator.clipboard && navigator.clipboard.writeText) {
            await navigator.clipboard.writeText(text);
            return true;
        }
        return false;
    };

    const uploadFile = async () => {
        const file = input.files && input.files[0];
        if (!file) {
            setStatus("Select an image to upload.", "error");
            return;
        }

        setStatus("Uploading...");
        output.hidden = true;

        try {
            const response = await fetch("/api/uploads", {
                method: "POST",
                headers: {
                    "Content-Type": file.type || "application/octet-stream",
                    "X-Upload-Filename": file.name || "image",
                },
                body: file,
            });

            const payload = await response.json().catch(() => null);
            if (!response.ok) {
                const message = payload && payload.error ? payload.error : "Upload failed.";
                setStatus(message, "error");
                return;
            }

            urlField.value = payload.url || "";
            markdownField.value = payload.markdown || "";
            output.hidden = false;
            setStatus("Upload complete.");
        } catch (err) {
            setStatus("Upload failed.", "error");
        }
    };

    button.addEventListener("click", uploadFile);
    input.addEventListener("change", () => {
        setStatus("");
        output.hidden = true;
    });

    if (copyUrl) {
        copyUrl.addEventListener("click", async () => {
            if (!urlField.value) {
                return;
            }
            const ok = await copyText(urlField.value);
            setStatus(ok ? "URL copied." : "Copy not supported.");
        });
    }

    if (copyMarkdown) {
        copyMarkdown.addEventListener("click", async () => {
            if (!markdownField.value) {
                return;
            }
            const ok = await copyText(markdownField.value);
            setStatus(ok ? "Markdown copied." : "Copy not supported.");
        });
    }
};

export { initUploads };
