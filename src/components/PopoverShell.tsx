// ── PopoverShell ───────────────────────────────────────────────────────────

type PopoverShellProps = {
	children: React.ReactNode;
};

/**
 * Liquid Glass 4-layer sandwich popover.
 * No extra wrappers -- raw liquidGlass structure to preserve SVG filter.
 */
export function PopoverShell({ children }: PopoverShellProps) {
	return (
		<div className="liquidGlass-wrapper popover">
			<div className="liquidGlass-effect" />
			<div className="liquidGlass-tint" />
			<div className="liquidGlass-shine" />
			<div className="liquidGlass-text">{children}</div>
		</div>
	);
}
