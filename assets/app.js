import { initTodoToggle } from "./features/todo_toggle.js";
import { initReorder } from "./features/reorder.js";
import { initPushSubscribe } from "./features/push_subscribe.js";
import { initServiceWorker } from "./features/sw_register.js";

const init = () => {
    initTodoToggle();
    initReorder();
    initPushSubscribe();
    initServiceWorker();
};

if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
} else {
    init();
}
