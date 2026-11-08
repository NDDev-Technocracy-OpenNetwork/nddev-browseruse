"use strict";
// The static legal page reads no browser state and makes no network requests.
const scheme = matchMedia("(prefers-color-scheme: dark)");
function theme() {
    document.documentElement.dataset.theme = scheme.matches ? "dark" : "light";
}
scheme.addEventListener("change", theme);
theme();
