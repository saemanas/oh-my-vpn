import { useNavigation } from "./stack-context";

// ── BackButton ─────────────────────────────────────────────────────────────

/**
 * Renders a back-navigation control at the top-left of a stack view.
 * Visible only when canGoBack is true. Calls pop() on click.
 *
 * Disabled during active transitions to prevent rapid-tap overlap.
 */
export function BackButton() {
  const { canGoBack, pop, previousTitle, isTransitioning } = useNavigation();

  if (!canGoBack) return null;

  return (
    <button
      type="button"
      className="back-button"
      onClick={pop}
      disabled={isTransitioning}
      aria-label="Go back"
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "var(--space-1)",
        padding: "var(--space-1) var(--space-2)",
        background: "none",
        border: "none",
        cursor: isTransitioning ? "not-allowed" : "pointer",
        color: "var(--color-text-secondary)",
        fontFamily: "var(--font-family-body)",
        fontSize: "var(--font-size-body-sm)",
        fontWeight: "var(--font-weight-medium)",
        borderRadius: "var(--radius-sm)",
        transition:
          "color var(--duration-fast) var(--easing-smooth), opacity var(--duration-fast) var(--easing-smooth)",
        opacity: isTransitioning ? 0.4 : 1,
      }}
    >
      <svg
        aria-hidden="true"
        width="16"
        height="16"
        viewBox="0 0 16 16"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        <path
          d="M10 13L5 8L10 3"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
      {previousTitle}
    </button>
  );
}
