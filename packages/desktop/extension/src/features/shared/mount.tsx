import { type ComponentType, createElement } from "react";
import { createRoot } from "react-dom/client";
import type { FrontendConfig } from "../../generated/types.gen.js";
import { loadLanguage } from "./i18n.js";

export async function mount(
  App: ComponentType<{ config: FrontendConfig }>,
  title: string,
): Promise<void> {
  const target = document.getElementById("root");
  if (!target) throw new Error("missing_ui_root");
  const root = createRoot(target, { onUncaughtError: () => failed() });
  function failed(): void {
    document.body.dataset.link = "down";
    root.render(
      <p id="setup" className="notice" role="alert">
        {chrome.i18n.getMessage("setupFailed")}
      </p>,
    );
  }
  try {
    const [, config] = await Promise.all([
      loadLanguage(title),
      fetch(chrome.runtime.getURL("public-settings.json")).then(async (response) => {
        if (!response.ok) throw new Error("configuration_unavailable");
        return (await response.json()) as FrontendConfig;
      }),
    ]);
    root.render(createElement(App, { config }));
  } catch {
    failed();
  }
}
