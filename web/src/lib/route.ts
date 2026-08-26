// Which screen is showing, held in the URL fragment so a reload keeps its place.

import { useEffect, useState } from "react";

export type Route = "home" | "editor";

function read(): Route {
  return window.location.hash.replace(/^#\/?/, "") === "editor" ? "editor" : "home";
}

export function go(route: Route): void {
  window.location.hash = route === "editor" ? "#/editor" : "#/";
}

export function useRoute(): Route {
  const [route, setRoute] = useState<Route>(read);

  useEffect(() => {
    const onChange = () => setRoute(read());
    window.addEventListener("hashchange", onChange);
    return () => window.removeEventListener("hashchange", onChange);
  }, []);

  return route;
}
