export interface ElementOptions {
  ariaLabel?: string;
  className?: string;
  dataset?: Record<string, string>;
  disabled?: boolean;
  id?: string;
  role?: string;
  text?: string;
  title?: string;
  type?: string;
}

export type Child = HTMLElement | SVGElement | Text | string;

export function createElement<K extends keyof HTMLElementTagNameMap>(
  tagName: K,
  options: ElementOptions = {},
  children: Child[] = [],
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tagName);

  applyOptions(element, options);
  appendChildren(element, children);

  return element;
}

export function createSvgIcon(pathData: string, label: string): SVGElement {
  const icon = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");

  icon.setAttribute("aria-hidden", "true");
  icon.setAttribute("class", "icon");
  icon.setAttribute("fill", "none");
  icon.setAttribute("stroke", "currentColor");
  icon.setAttribute("stroke-linecap", "round");
  icon.setAttribute("stroke-linejoin", "round");
  icon.setAttribute("stroke-width", "2");
  icon.setAttribute("viewBox", "0 0 24 24");
  path.setAttribute("d", pathData);
  path.setAttribute("data-icon", label);
  icon.append(path);

  return icon;
}

export function replaceChildren(element: HTMLElement, children: Child[]): void {
  element.replaceChildren();
  appendChildren(element, children);
}

function appendChildren(element: HTMLElement, children: Child[]): void {
  for (const child of children) {
    if (typeof child === "string") {
      element.append(document.createTextNode(child));
    } else {
      element.append(child);
    }
  }
}

function applyOptions(element: HTMLElement, options: ElementOptions): void {
  if (options.className) {
    element.className = options.className;
  }

  if (options.id) {
    element.id = options.id;
  }

  if (options.text !== undefined) {
    element.textContent = options.text;
  }

  if (options.title) {
    element.title = options.title;
  }

  if (options.role) {
    element.setAttribute("role", options.role);
  }

  if (options.ariaLabel) {
    element.setAttribute("aria-label", options.ariaLabel);
  }

  if (options.type && element instanceof HTMLButtonElement) {
    element.setAttribute("type", options.type);
  }

  if (options.disabled !== undefined && element instanceof HTMLButtonElement) {
    element.disabled = options.disabled;
  }

  if (options.dataset) {
    for (const [key, value] of Object.entries(options.dataset)) {
      element.dataset[key] = value;
    }
  }
}
