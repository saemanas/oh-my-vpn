import { useEffect, useState } from "react";
import "./StackNavigator.css";
import { useNavigation } from "./stack-context";
import type { ViewEntry } from "./stack-context";

// ── Types ──────────────────────────────────────────────────────────────────

type RenderedView = {
  entry: ViewEntry;
  role: "entering" | "exiting" | "idle";
};

// ── StackNavigator ─────────────────────────────────────────────────────────

/**
 * Renders the active stack view with CSS push/pop transition animations.
 * Reads all state from NavigationContext -- accepts no props.
 *
 * Transition lifecycle:
 *   1. isTransitioning=true  → render both entering + exiting views
 *   2. Next render tick      → add `.stack-active` to trigger CSS transition
 *   3. transitionend fires   → call onTransitionEnd(), remove exiting view
 */
export function StackNavigator() {
  const { stack, direction, isTransitioning, onTransitionEnd } =
    useNavigation();

  // Track the two views being rendered during a transition
  const [views, setViews] = useState<RenderedView[]>(() => [
    { entry: stack[stack.length - 1], role: "idle" },
  ]);

  // Delayed active class: applied one rAF after mount to trigger CSS transition
  const [isActiveTriggered, setIsActiveTriggered] = useState(false);

  useEffect(() => {
    if (!isTransitioning) {
      // Reset: only show the current top of stack in idle state
      setIsActiveTriggered(false);
      setViews([{ entry: stack[stack.length - 1], role: "idle" }]);
      return;
    }

    // Capture the view that is leaving
    if (direction === "push") {
      // stack already has the new entry appended; previous is second-to-last
      const enteringEntry = stack[stack.length - 1];
      const exitingEntry = stack[stack.length - 2];
      setViews([
        { entry: exitingEntry, role: "exiting" },
        { entry: enteringEntry, role: "entering" },
      ]);
    } else {
      // pop: current top will exit, the one below it enters
      const exitingEntry = stack[stack.length - 1];
      const enteringEntry = stack[stack.length - 2];
      setViews([
        { entry: enteringEntry, role: "entering" },
        { entry: exitingEntry, role: "exiting" },
      ]);
    }

    // Trigger `.stack-active` on next animation frame so CSS sees both states
    const rafId = requestAnimationFrame(() => {
      setIsActiveTriggered(true);
    });

    return () => cancelAnimationFrame(rafId);
  }, [isTransitioning, direction, stack]);

  function buildClassName(role: RenderedView["role"]): string {
    const base = "stack-view";
    if (role === "idle") return base;

    const dirClass =
      role === "entering"
        ? `stack-entering-${direction}`
        : `stack-exiting-${direction}`;

    const activeClass = isActiveTriggered ? "stack-active" : "";
    return [base, dirClass, activeClass].filter(Boolean).join(" ");
  }

  function handleTransitionEnd(role: RenderedView["role"]) {
    // Only the exiting view's transition signals completion
    if (role === "exiting") {
      onTransitionEnd();
    }
  }

  return (
    <div className="stack-navigator" aria-live="polite">
      {views.map(({ entry, role }) => (
        <div
          key={entry.id}
          className={buildClassName(role)}
          // Only exiting views are inert -- idle (resting) view must stay interactive
          inert={role === "exiting" ? true : undefined}
          onTransitionEnd={() => handleTransitionEnd(role)}
        >
          {entry.component}
        </div>
      ))}
    </div>
  );
}
