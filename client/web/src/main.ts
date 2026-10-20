import { initTheme } from "@nimbus/shared/theme";
import { mount } from "svelte";
import { registerSW } from "virtual:pwa-register";
import App from "./App.svelte";
import "./app.css";

initTheme();

registerSW({ immediate: true });

const target = document.getElementById("app");
if (!target) {
  throw new Error("missing app root");
}

export default mount(App, { target });
