import { render } from "solid-js/web";
import { Router, Route } from "@solidjs/router";
import App from "./App";
import Recorder from "./pages/Recorder";
import LibraryPage from "./pages/Library";
import ExportPage from "./pages/Export";
import SettingsPage from "./pages/Settings";

render(
  () => (
    <Router root={App}>
      <Route path="/" component={Recorder} />
      <Route path="/library" component={LibraryPage} />
      <Route path="/export" component={ExportPage} />
      <Route path="/settings" component={SettingsPage} />
    </Router>
  ),
  document.getElementById("root")!
);
