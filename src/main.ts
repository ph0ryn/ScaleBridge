import { mountApp } from "./app/app";
import "./styles/app.css";

const rootElement = document.querySelector<HTMLElement>("#app");

if (!rootElement) {
  throw new Error("missing #app root element");
}

mountApp(rootElement);
