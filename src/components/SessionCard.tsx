import type { Provider, SessionStatus } from "../types/ipc";

// ── Types ──────────────────────────────────────────────────────────────────

type SessionCardProps = {
	session: SessionStatus;
};

// ── Helpers ────────────────────────────────────────────────────────────────

/**
 * Convert an ISO 3166-1 alpha-2 country code to a flag emoji.
 * Each letter maps to a regional indicator symbol (U+1F1E6--U+1F1FF).
 *
 * Example: "DE" → 🇩🇪
 */
function countryCodeToFlag(countryCode: string): string {
	return [...countryCode.toUpperCase()]
		.map((char) => String.fromCodePoint(0x1f1e6 + char.charCodeAt(0) - 65))
		.join("");
}

/**
 * Extract the ISO country code from a display name like "Falkenstein, DE".
 * Takes the last 2 characters after the final ", " separator.
 * Falls back to "UN" (United Nations flag placeholder) if format is unexpected.
 */
function extractCountryCode(displayName: string): string {
	const parts = displayName.split(", ");
	const code = parts[parts.length - 1];
	return code && code.length === 2 ? code : "UN";
}

/** Format elapsed seconds as HH:MM:SS. */
function formatElapsedTime(totalSeconds: number): string {
	const hours = Math.floor(totalSeconds / 3600);
	const minutes = Math.floor((totalSeconds % 3600) / 60);
	const seconds = totalSeconds % 60;
	return [hours, minutes, seconds]
		.map((unit) => String(unit).padStart(2, "0"))
		.join(":");
}

/** Format accumulated cost as $X.XXX. */
function formatCost(cost: number): string {
	return `$${cost.toFixed(3)}`;
}

/** Map provider enum to display name. */
function formatProviderName(provider: Provider): string {
	const names: Record<Provider, string> = {
		hetzner: "Hetzner",
		aws: "AWS",
		gcp: "GCP",
	};
	return names[provider];
}

// ── Component ──────────────────────────────────────────────────────────────

/**
 * Pure presentational component displaying active session information.
 *
 * Layout:
 *   - IP address (SF Mono 22px semibold)
 *   - Region with flag emoji + provider name (13px)
 *   - Divider
 *   - Metrics row: elapsed time + accumulated cost (SF Mono 13px)
 *
 * Uses the Liquid Glass 4-layer sandwich with success tint (opacity 0.10).
 */
export function SessionCard({ session }: SessionCardProps) {
	const countryCode = extractCountryCode(session.regionDisplayName);
	const flag = countryCodeToFlag(countryCode);
	const providerName = formatProviderName(session.provider);
	const elapsed = formatElapsedTime(session.elapsedSeconds);
	const cost = formatCost(session.accumulatedCost);

	return (
		<div className="liquidGlass-wrapper session-card">
			<div className="liquidGlass-effect" />
			<div className="liquidGlass-tint" />
			<div className="liquidGlass-shine" />
			<div className="liquidGlass-text session-card__content">
				<span className="session-card__ip">{session.serverIp}</span>
				<span className="session-card__region">
					<span aria-hidden="true">{flag}</span>{" "}
					{session.regionDisplayName} &middot; {providerName}
				</span>
				<hr className="session-card__divider" />
				<div className="session-card__metrics">
					<span className="session-card__metric">
						<span aria-hidden="true">⏱</span> {elapsed}
					</span>
					<span className="session-card__metric">
						<span aria-hidden="true">$</span> {cost}
					</span>
				</div>
			</div>
		</div>
	);
}
