const getDocId = () => {
    const path = window.location.pathname;
    if (!path.startsWith("/doc/")) {
        return "";
    }
    return decodeURIComponent(path.slice(5));
};

const toggleTask = async (input, docId) => {
    const taskIndex = Number.parseInt(input.dataset.taskIndex, 10);
    if (!Number.isFinite(taskIndex)) {
        return;
    }
    const desired = input.checked;
    input.disabled = true;
    try {
        const body = new URLSearchParams({
            doc_id: docId,
            task_index: String(taskIndex),
            checked: desired ? "true" : "false",
        });
        const response = await fetch("/api/doc/toggle-task", {
            method: "POST",
            headers: { "Content-Type": "application/x-www-form-urlencoded" },
            body,
        });
        if (!response.ok) {
            throw new Error("Failed to save");
        }
    } catch (err) {
        input.checked = !desired;
        console.error(err);
    } finally {
        input.disabled = false;
    }
};

export const initTodoToggle = () => {
    const docId = getDocId();
    if (!docId) {
        return;
    }
    document
        .querySelectorAll('input.todo-checkbox[data-task-index]')
        .forEach((input) => {
            input.addEventListener("change", () => toggleTask(input, docId));
        });
};
