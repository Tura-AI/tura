import ChevronRight from "lucide-solid/icons/chevron-right";
import type { JSX } from "solid-js";
import { classNames } from "../../state/format";

export function RailSectionTitle(props: {
  className?: string;
  icon?: JSX.Element;
  expanded: boolean;
  children: JSX.Element;
  onToggle: () => void;
}) {
  return (
    <button
      class={classNames("section-title", props.className)}
      type="button"
      onClick={props.onToggle}
    >
      {props.icon}
      <span>{props.children}</span>
      <RailDisclosure expanded={props.expanded} />
    </button>
  );
}

export function RailDisclosure(props: { expanded: boolean }) {
  return (
    <span
      class={classNames("rail-disclosure", props.expanded && "expanded")}
      aria-hidden="true"
    >
      <ChevronRight size={13} strokeWidth={1.8} />
    </span>
  );
}
