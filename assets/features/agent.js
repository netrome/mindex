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
};

const submitDirective = async (docId, afterLine, directive, disableControls) => {
    if (!directive) return;
    disableControls(true);

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
        disableControls(false);
    }
};

const createForm = (block, docId) => {
    collapseAll();

    const btn = block.querySelector(".agent-insert-btn");
    btn.style.display = "none";

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
    block.after(form);

    textarea.focus();

    const afterLine = block.dataset.afterLine;
    const doSubmit = () => {
        submitDirective(docId, afterLine, textarea.value.trim(), (disabled) => {
            submit.disabled = disabled;
            textarea.disabled = disabled;
        });
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
    document.querySelectorAll(".agent-block .agent-insert-btn").forEach((btn) => {
        btn.addEventListener("click", () => {
            createForm(btn.closest(".agent-block"), docId);
        });
    });
};

const initBottomInput = (docId) => {
    const bottom = document.querySelector(".agent-bottom-input");
    if (!bottom) return;

    const textarea = bottom.querySelector(".agent-insert-input");
    const submit = bottom.querySelector(".agent-insert-submit");
    if (!textarea || !submit) return;

    const afterLine = bottom.dataset.afterLine;
    const doSubmit = () => {
        submitDirective(docId, afterLine, textarea.value.trim(), (disabled) => {
            submit.disabled = disabled;
            textarea.disabled = disabled;
        });
    };

    submit.addEventListener("click", doSubmit);
    textarea.addEventListener("keydown", (e) => {
        if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
            doSubmit();
        }
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

const removeInteraction = async (btn, docId) => {
    const block = btn.closest(".agent-block-directive");
    const directiveLine = block.dataset.directiveLine;
    btn.disabled = true;

    try {
        const body = new URLSearchParams({
            doc_id: docId,
            directive_line: directiveLine,
        });
        const response = await fetch("/api/d/remove-magent-interaction", {
            method: "POST",
            headers: { "Content-Type": "application/x-www-form-urlencoded" },
            body,
        });
        if (!response.ok) {
            throw new Error("Failed to remove interaction");
        }
        window.location.reload();
    } catch (err) {
        console.error(err);
        btn.disabled = false;
    }
};

const initRemoveButtons = (docId) => {
    document.querySelectorAll(".agent-block-directive").forEach((block) => {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "magent-remove-btn";
        btn.textContent = "\u00d7";
        btn.title = "Remove interaction";
        block.appendChild(btn);
        btn.addEventListener("click", () => removeInteraction(btn, docId));
    });
};

export const initAgent = () => {
    const docId = getDocId();
    if (!docId) return;

    initInsertPoints(docId);
    initBottomInput(docId);
    initAcceptButtons(docId);
    initRemoveButtons(docId);
};
