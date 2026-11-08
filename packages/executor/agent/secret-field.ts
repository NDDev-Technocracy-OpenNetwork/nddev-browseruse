/** Serialized into the page; no imported state or user-entered values leave this check. */
export function secretField(element: Element): boolean {
  const input = element as HTMLInputElement;
  const fill = (element.getAttribute("autocomplete") ?? "").toLowerCase();
  return (
    input.type === "password" ||
    /one-time-code|cc-number|cc-csc|cc-exp|current-password|new-password/.test(fill)
  );
}
