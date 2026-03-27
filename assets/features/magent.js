const getDocId = () => {
    const path = window.location.pathname;
    if (!path.startsWith("/d/")) {
        return "";
    }
    return decodeURIComponent(path.slice(3));
};

const acceptEdit = async (button, docId) => {
    const editDiv = button.closest(".magent-edit");
    if (!editDiv) {
        return;
    }
    const editIndex = Number.parseInt(editDiv.dataset.editIndex, 10);
    if (!Number.isFinite(editIndex)) {
        return;
    }
    button.disabled = true;
    button.textContent = "Accepting\u2026";
    try {
        const body = new URLSearchParams({
            doc_id: docId,
            edit_index: String(editIndex),
        });
        const response = await fetch("/api/d/accept-magent-edit", {
            method: "POST",
            headers: { "Content-Type": "application/x-www-form-urlencoded" },
            body,
        });
        if (!response.ok) {
            throw new Error("Failed to accept edit");
        }
        editDiv.dataset.status = "accepted";
        button.textContent = "Accepted";
    } catch (err) {
        button.disabled = false;
        button.textContent = "Accept";
        console.error(err);
    }
};

const initMagent = () => {
    const docId = getDocId();
    if (!docId) {
        return;
    }
    document
        .querySelectorAll('.magent-edit[data-status="proposed"]')
        .forEach((editDiv) => {
            const button = document.createElement("button");
            button.type = "button";
            button.className = "magent-accept-btn";
            button.textContent = "Accept";
            button.addEventListener("click", () => acceptEdit(button, docId));
            editDiv.appendChild(button);
        });
};

initMagent();
