import { Route } from "@solidjs/router";
import type { JSX } from "solid-js";
import { PRODUCT_ROUTE_PATHS } from "./route-paths";

export function AppRoutes(props: { component: () => JSX.Element }) {
  return (
    <>
      {PRODUCT_ROUTE_PATHS.map((path) => (
        <Route path={path} component={props.component} />
      ))}
    </>
  );
}
