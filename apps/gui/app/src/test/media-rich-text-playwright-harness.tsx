/* @jsxImportSource solid-js */
import { render } from "solid-js/web";
import { RichText } from "../conversation/message-rich-text";
import "../styles/index.css";

const params = new URLSearchParams(window.location.search);
const workspaceDirectory = params.get("workspace") ?? undefined;
const paths = params.getAll("path").filter(Boolean);
const text = paths.map((path) => `[MEDIA:${path}:MEDIA]`).join("\n");
const root = document.getElementById("root");

if (!root) {
  throw new Error("media harness root was not found");
}

render(() => <RichText text={text} workspaceDirectory={workspaceDirectory} />, root);
