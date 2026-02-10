import { createApp } from "vue";
import { createRouter, createWebHashHistory } from "vue-router";
import App from "./App.vue";
import "./styles/main.css";

import PlanViewer from "./views/PlanViewer.vue";
import SqlEditorView from "./views/SqlEditorView.vue";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", redirect: "/plan-viewer" },
    { path: "/plan-viewer", component: PlanViewer, name: "plan-viewer" },
    { path: "/sql-editor", component: SqlEditorView, name: "sql-editor" },
  ],
});

createApp(App).use(router).mount("#app");
