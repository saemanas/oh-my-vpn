import { BackButton } from "../navigation/BackButton";

// ── DetailView ─────────────────────────────────────────────────────────────

/**
 * Generic detail placeholder -- used as a navigation target in development
 * and as a template for future detail views.
 */
export function DetailView() {
	return (
		<div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
			<BackButton />
			<h2 style={{ margin: 0, fontSize: "20px", fontWeight: 600 }}>Detail</h2>
			<p
				style={{
					margin: 0,
					fontSize: "14px",
					color: "rgba(0,0,0,0.6)",
					lineHeight: 1.6,
				}}
			>
				Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do
				eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad
				minim veniam, quis nostrud exercitation ullamco laboris.
			</p>
		</div>
	);
}
