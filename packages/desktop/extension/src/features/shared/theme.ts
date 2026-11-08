let media: [MediaQueryList, MediaQueryList] | undefined;

export function watchTheme(): void {
  function apply(): void {
    const dark = media?.[0].matches;
    const contrast = media?.[1].matches;
    document.documentElement.dataset.theme = `${dark ? "dark" : "light"}${contrast ? "-contrast" : ""}`;
  }
  if (!media) {
    media = [matchMedia("(prefers-color-scheme: dark)"), matchMedia("(prefers-contrast: more)")];
    for (const query of media) query.addEventListener("change", apply);
  }
  apply();
}
