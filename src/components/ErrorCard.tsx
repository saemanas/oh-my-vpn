import type { ReactNode } from "react";
import "./ErrorCard.css";

// ── Types ──────────────────────────────────────────────────────────────────

type ErrorCardVariant = "error" | "warning";

type ErrorCardProps = {
	/** Error or warning message to display. */
	message: string;
	/** Optional secondary description below the message. */
	description?: string;
	/** Visual variant controlling tint and text color. Default: "error". */
	variant?: ErrorCardVariant;
	/** Action slot -- typically GlassButtons for Cancel, Retry, etc. */
	children?: ReactNode;
};

// ── Component ──────────────────────────────────────────────────────────────

/**
 * Reusable error/warning card using the Liquid Glass 4-layer sandwich.
 *
 * Variants:
 *   - `error` -- red tint, for provisioning failures and generic errors
 *   - `warning` -- amber tint, for persistent destruction failures
 *
 * Children are rendered in a flex row as the action slot (buttons).
 */
export function ErrorCard({
	message,
	description,
	variant = "error",
	children,
}: ErrorCardProps) {
	return (
		<div
			className={`error-card error-card--${variant} liquidGlass-wrapper`}
			role="alert"
		>
			<div className="liquidGlass-effect" />
			<div className="liquidGlass-tint" />
			<div className="liquidGlass-shine" />
			<div className="liquidGlass-text error-card__content">
				<p className="error-card__message">{message}</p>
				{description && (
					<p className="error-card__description">{description}</p>
				)}
				{children && <div className="error-card__actions">{children}</div>}
			</div>
		</div>
	);
}
