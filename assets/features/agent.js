const getDocId = () => {
    const page = document.querySelector(".agent-page[data-doc-id]");
    return page ? page.dataset.docId : "";
};

const collapseAll = () => {
    for (const form of document.querySelectorAll(".agent-insert-form")) {
        form.remove();
    }
    for (const btn of document.querySelectorAll(".agent-insert-btn")) {
        btn.style.display = "";
    }
    for (const point of document.querySelectorAll(".agent-insert-point")) {
        point.classList.remove("agent-insert-expanded");
    }
};

const createForm = (insertPoint, docId) => {
    collapseAll();

    const btn = insertPoint.querySelector(".agent-insert-btn");
    btn.style.display = "none";
    insertPoint.classList.add("agent-insert-expanded");

    const form = document.createElement("div");
    form.className = "agent-insert-form";

    const textarea = document.createElement("textarea");
    textarea.className = "agent-insert-input";
    textarea.placeholder = "What would you like to ask?";
    textarea.rows = 2;

    const actions = document.createElement("div");
    actions.className = "agent-insert-actions";

    const submit = document.createElement("button");
    submit.type = "button";
    submit.className = "agent-insert-submit";
    submit.textContent = "Send";

    actions.appendChild(submit);
    form.appendChild(textarea);
    form.appendChild(actions);
    insertPoint.appendChild(form);

    textarea.focus();

    const doSubmit = async () => {
        const directive = textarea.value.trim();
        if (!directive) return;

        const afterLine = insertPoint.dataset.afterLine;
        submit.disabled = true;
        textarea.disabled = true;

        try {
            const body = new URLSearchParams({
                doc_id: docId,
                after_line: afterLine,
                directive,
            });
            const response = await fetch("/api/d/insert-magent-directive", {
                method: "POST",
                headers: { "Content-Type": "application/x-www-form-urlencoded" },
                body,
            });
            if (!response.ok) {
                throw new Error("Failed to insert directive");
            }
            window.location.reload();
        } catch (err) {
            console.error(err);
            submit.disabled = false;
            textarea.disabled = false;
        }
    };

    submit.addEventListener("click", doSubmit);
    textarea.addEventListener("keydown", (e) => {
        if (e.key === "Escape") {
            collapseAll();
        } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
            doSubmit();
        }
    });
};

const initInsertPoints = (docId) => {
    document.querySelectorAll(".agent-insert-btn").forEach((btn) => {
        btn.addEventListener("click", () => {
            createForm(btn.closest(".agent-insert-point"), docId);
        });
    });
};

const acceptEdit = async (btn, docId) => {
    const edit = btn.closest(".magent-edit");
    const editIndex = edit.dataset.editIndex;
    btn.disabled = true;

    try {
        const body = new URLSearchParams({
            doc_id: docId,
            edit_index: editIndex,
        });
        const response = await fetch("/api/d/accept-magent-edit", {
            method: "POST",
            headers: { "Content-Type": "application/x-www-form-urlencoded" },
            body,
        });
        if (!response.ok) {
            throw new Error("Failed to accept edit");
        }
        edit.dataset.status = "accepted";
        btn.textContent = "Accepted";
    } catch (err) {
        console.error(err);
        btn.disabled = false;
    }
};

const initAcceptButtons = (docId) => {
    document.querySelectorAll('.magent-edit[data-status="proposed"]').forEach((edit) => {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "magent-accept-btn";
        btn.textContent = "Accept";
        edit.appendChild(btn);
        btn.addEventListener("click", () => acceptEdit(btn, docId));
    });
};

export const initAgent = () => {
    const docId = getDocId();
    if (!docId) return;

    initInsertPoints(docId);
    initAcceptButtons(docId);
};
