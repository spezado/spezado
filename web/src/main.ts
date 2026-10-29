// SPDX-License-Identifier: AGPL-3.0-or-later
import { mount } from "svelte";
import "./brand.css";
import App from "./App.svelte";
import { app, setTheme } from "./lib/state.svelte";

setTheme(app.theme);
document.documentElement.lang = app.lang;

export default mount(App, { target: document.getElementById("app")! });
