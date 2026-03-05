import "./GlassInput.css";

// ── Types ──────────────────────────────────────────────────────────────────

type GlassInputProps = {
	/** Current input value. */
	value: string;
	/** Change handler. */
	onChange: (value: string) => void;
	/** Blur handler -- called when the input loses focus. */
	onBlur?: () => void;
	/** Placeholder text displayed when empty. */
	placeholder?: string;
	/** HTML input type. Default: 'text'. */
	type?: "text" | "password" | "email" | "url";
	/** Error message -- triggers error state (red tint + message below). */
	error?: string;
	/** When true, shows a green check icon (success state). */
	success?: boolean;
	/** When true, renders at 0.4 opacity with pointer-events disabled. */
	disabled?: boolean;
	/** Additional class names merged onto the outermost element. */
	className?: string;
};

// ── Component ──────────────────────────────────────────────────────────────

/**
 * Reusable Liquid Glass input.
 *
 * Uses the 4-layer sandwich pattern from liquid-glass.css:
 *   wrapper (shadow + transition)
 *   → effect (backdrop blur + SVG distortion)
 *   → tint (state colour overlay)
 *   → shine (inset specular highlight)
 *   → content (input element)
 *
 * States: default, focus (enhanced shine), error (red tint + inline error),
 * success (green check icon), disabled (opacity 0.4).
 */
export function GlassInput({
	value,
	onChange,
	onBlur,
	placeholder,
	type = "text",
	error,
	success = false,
	disabled = false,
	className = "",
}: GlassInputProps) {
	const hasError = Boolean(error);

	const classes = [
		"liquidGlass-wrapper",
		"glass-input",
		hasError ? "glass-input--error" : "",
		success ? "glass-input--success" : "",
		className,
	]
		.filter(Boolean)
		.join(" ");

	return (
		<div className="glass-input-container">
			<div className={classes}>
				<div className="liquidGlass-effect" />
				<div className="liquidGlass-tint" />
				<div className="liquidGlass-shine" />
				<div className="liquidGlass-text glass-input__content">
					<input
						type={type}
						value={value}
						onChange={(event) => onChange(event.target.value)}
						onBlur={onBlur}
						placeholder={placeholder}
						disabled={disabled}
						className="glass-input__field"
						aria-invalid={hasError}
						aria-describedby={hasError ? "glass-input-error" : undefined}
					/>
					{success && !hasError && (
						<span className="glass-input__check" role="img" aria-label="Valid">
							✓
						</span>
					)}
				</div>
			</div>
			{hasError && (
				<p className="glass-input__error" id="glass-input-error" role="alert">
					{error}
				</p>
			)}
		</div>
	);
}
