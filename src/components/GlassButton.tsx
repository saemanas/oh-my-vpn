import "./GlassButton.css";

// ── Types ──────────────────────────────────────────────────────────────────

type GlassButtonVariant = "success" | "error" | "neutral" | "warning" | "info";

type GlassButtonProps = {
	/** Visual variant controlling tint and text color. Default: 'neutral'. */
	variant?: GlassButtonVariant;
	/** When true, renders at 0.4 opacity with pointer-events disabled. */
	disabled?: boolean;
	/**
	 * When true, shows a CSS spinner alongside children, overrides tint to
	 * warning, and disables clicks. Implies disabled interaction.
	 */
	loading?: boolean;
	onClick?: () => void;
	children: React.ReactNode;
	/** Additional class names merged onto the outermost element. */
	className?: string;
	type?: "button" | "submit";
};

// ── Component ──────────────────────────────────────────────────────────────

/**
 * Reusable Liquid Glass button.
 *
 * Uses the 4-layer sandwich pattern from liquid-glass.css:
 *   wrapper (shadow + transition)
 *   → effect (backdrop blur + SVG distortion)
 *   → tint (variant colour overlay)
 *   → shine (inset specular highlight)
 *   → text (content)
 *
 * Hover: scale(1.02) + padding expand (liquid-glass.css).
 * Active: scale(0.97).
 * Disabled: opacity 0.4, no hover/active transforms.
 * Loading: warning tint + spinner, clicks blocked.
 */
export function GlassButton({
	variant = "neutral",
	disabled = false,
	loading = false,
	onClick,
	children,
	className = "",
	type = "button",
}: GlassButtonProps) {
	const isDisabled = disabled || loading;

	const classes = [
		"liquidGlass-wrapper",
		"glass-btn",
		`glass-btn--${variant}`,
		loading ? "glass-btn--loading" : "",
		className,
	]
		.filter(Boolean)
		.join(" ");

	return (
		<button
			type={type}
			className={classes}
			onClick={isDisabled ? undefined : onClick}
			disabled={isDisabled}
			aria-busy={loading}
			style={{ border: "none", background: "none" }}
		>
			<div className="liquidGlass-effect" />
			<div className="liquidGlass-tint" />
			<div className="liquidGlass-shine" />
			<div className="liquidGlass-text">
				{loading && <span className="glass-btn-spinner" aria-hidden="true" />}
				{children}
			</div>
		</button>
	);
}
