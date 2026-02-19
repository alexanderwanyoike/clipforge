import { type ParentProps } from "solid-js";
import { A } from "@solidjs/router";

export default function App(props: ParentProps) {
  return (
    <div class="app">
      <nav class="sidebar">
        <div class="logo">ClipForge</div>
        <A href="/" class="nav-link" activeClass="active" end>
          Record
        </A>
        <A href="/library" class="nav-link" activeClass="active">
          Library
        </A>
        <A href="/export" class="nav-link" activeClass="active">
          Export
        </A>
        <A href="/settings" class="nav-link" activeClass="active">
          Settings
        </A>
      </nav>
      <main class="content">{props.children}</main>
    </div>
  );
}
