import { initTodoToggle } from "./features/todo_toggle.js";
import { initReorder } from "./features/reorder.js";
import { initPushSubscribe } from "./features/push_subscribe.js";
import { initServiceWorker } from "./features/sw_register.js";
import { initUploads } from "./features/uploads.js";
import { initEditorPasteUploads } from "./features/editor_paste_upload.js";

const init = () => {
    initTodoToggle();
    initReorder();
    initPushSubscribe();
    initServiceWorker();
    initUploads();
    initEditorPasteUploads();
};

if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
} else {
    init();
}
