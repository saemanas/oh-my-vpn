import "./RegionList.css";
import type { RegionInfo } from "../types/ipc";

// ── Types ──────────────────────────────────────────────────────────────────

type RegionListProps = {
	regions: RegionInfo[];
	selectedRegion: string | null;
	onSelect: (region: string) => void;
	isLoading: boolean;
};

// ── Helpers ────────────────────────────────────────────────────────────────

/**
 * Convert an ISO 3166-1 alpha-2 country code to a flag emoji.
 * Each letter maps to a regional indicator symbol (U+1F1E6–U+1F1FF).
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

/**
 * Format a raw hourly cost number as a USD string: "$X.XXX/hr".
 * Uses 3 decimal places to surface sub-cent differences.
 */
function formatHourlyCost(hourlyCost: number): string {
	return `$${hourlyCost.toFixed(3)}/hr`;
}

// ── Skeleton ───────────────────────────────────────────────────────────────

function SkeletonRows() {
	return (
		<>
			{[0, 1, 2].map((i) => (
				<div
					key={i}
					className="region-row region-row--skeleton"
					aria-hidden="true"
				>
					<div className="region-row__skeleton-flag" />
					<div className="region-row__skeleton-body">
						<div className="region-row__skeleton-name" />
						<div className="region-row__skeleton-instance" />
					</div>
					<div className="region-row__skeleton-cost" />
				</div>
			))}
		</>
	);
}

// ── Component ──────────────────────────────────────────────────────────────

/**
 * Scrollable region list for the disconnected view.
 *
 * Shows a shimmer skeleton while `isLoading` is true, then renders each
 * RegionInfo as an interactive row with:
 *   - Country flag emoji (derived from displayName country code)
 *   - Region display name + instance type (caption, secondary color)
 *   - Hourly cost right-aligned in SF Mono
 *
 * Each row uses the Liquid Glass 4-layer sandwich pattern for glass tint
 * hover and selected states. Hidden scrollbar (webkit + Firefox).
 *
 * Keyboard accessible: rows are focusable; Enter and Space trigger onSelect.
 */
export function RegionList({
	regions,
	selectedRegion,
	onSelect,
	isLoading,
}: RegionListProps) {
	if (isLoading) {
		return (
			<output className="region-list" aria-label="Loading regions">
				<SkeletonRows />
			</output>
		);
	}

	return (
		<div
			className="region-list"
			role="listbox"
			aria-label="Region"
			tabIndex={-1}
			aria-activedescendant={
				selectedRegion ? `region-row-${selectedRegion}` : undefined
			}
		>
			{regions.map((info) => {
				const isSelected = info.region === selectedRegion;
				const countryCode = extractCountryCode(info.displayName);
				const flag = countryCodeToFlag(countryCode);
				const cost = formatHourlyCost(info.hourlyCost);

				const classes = [
					"liquidGlass-wrapper",
					"region-row",
					isSelected ? "region-row--selected" : "",
				]
					.filter(Boolean)
					.join(" ");

				return (
					<div
						key={info.region}
						id={`region-row-${info.region}`}
						className={classes}
						role="option"
						aria-selected={isSelected}
						tabIndex={0}
						onClick={() => onSelect(info.region)}
						onKeyDown={(e) => {
							if (e.key === "Enter" || e.key === " ") {
								e.preventDefault();
								onSelect(info.region);
							}
						}}
					>
						<div className="liquidGlass-effect" />
						<div className="liquidGlass-tint" />
						<div className="liquidGlass-shine" />
						<div className="liquidGlass-text region-row__content">
							<span className="region-row__flag" aria-hidden="true">
								{flag}
							</span>
							<span className="region-row__info">
								<span className="region-row__name">{info.displayName}</span>
								<span className="region-row__instance">
									{info.instanceType}
								</span>
							</span>
							<span className="region-row__cost">{cost}</span>
						</div>
					</div>
				);
			})}
		</div>
	);
}
