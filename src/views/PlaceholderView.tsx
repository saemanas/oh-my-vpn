import { BackButton } from "../navigation/BackButton";
import { useNavigation } from "../navigation/stack-context";

// ── DetailView ─────────────────────────────────────────────────────────────

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

// ── HomeView ───────────────────────────────────────────────────────────────

export function HomeView() {
	const { push } = useNavigation();

	function onClickPushDetail() {
		push("detail", "Detail", <DetailView />);
	}

	return (
		<div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
			<h2 style={{ margin: 0, fontSize: "20px", fontWeight: 600 }}>Home</h2>
			<p
				style={{
					margin: 0,
					fontSize: "14px",
					color: "rgba(0,0,0,0.6)",
					lineHeight: 1.6,
				}}
			>
				Oh My VPN popover shell. Push the button below to test navigation.
			</p>
			<button
				type="button"
				className="liquidGlass-wrapper glass-btn"
				onClick={onClickPushDetail}
				style={{ border: "none", background: "none" }}
			>
				<div className="liquidGlass-effect" />
				<div className="liquidGlass-tint" />
				<div className="liquidGlass-shine" />
				<div className="liquidGlass-text">Push Detail View</div>
			</button>
		</div>
	);
}
