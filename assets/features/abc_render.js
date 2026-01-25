const renderAbcBlock = (node) => {
    const source = node.textContent || "";
    if (!source.trim()) {
        return;
    }

    node.textContent = "";

    if (!window.ABCJS || typeof window.ABCJS.renderAbc !== "function") {
        node.textContent = source;
        node.classList.add("abc-render-error");
        return;
    }

    try {
        window.ABCJS.renderAbc(node, source, {
            responsive: "resize",
        });
    } catch (error) {
        node.textContent = source;
        node.classList.add("abc-render-error");
    }
};

const initAbcRender = () => {
    const nodes = document.querySelectorAll(".abc-notation");
    if (!nodes.length) {
        return;
    }

    nodes.forEach(renderAbcBlock);
};

export { initAbcRender };
