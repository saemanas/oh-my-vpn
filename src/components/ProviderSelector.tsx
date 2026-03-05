import "./ProviderSelector.css";
import type { Provider, ProviderInfo } from "../types/ipc";

// ── Types ──────────────────────────────────────────────────────────────────

type ProviderSelectorProps = {
	providers: ProviderInfo[];
	selectedProvider: Provider;
	onSelect: (provider: Provider) => void;
};

// ── Helpers ────────────────────────────────────────────────────────────────

/** Capitalize first letter of a provider name for display. */
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
 * Provider selector list for the disconnected view.
 *
 * Renders NOTHING when only one provider is available -- single-provider
 * setups have no switching to do.
 *
 * Each row shows "Provider · account-label" with a right-arrow chevron.
 * Rows with `status !== "valid"` are rendered at reduced opacity (disabled).
 *
 * Uses the Liquid Glass 4-layer sandwich pattern:
 *   wrapper → effect → tint → shine → text
 */
export function ProviderSelector({
	providers,
	selectedProvider,
	onSelect,
}: ProviderSelectorProps) {
	if (providers.length <= 1) {
		return null;
	}

	return (
		<div className="provider-selector" role="listbox" aria-label="Provider">
			{providers.map((info) => {
				const isSelected = info.provider === selectedProvider;
				const isDisabled = info.status !== "valid";

				const classes = [
					"liquidGlass-wrapper",
					"provider-row",
					isSelected ? "provider-row--selected" : "",
					isDisabled ? "provider-row--disabled" : "",
				]
					.filter(Boolean)
					.join(" ");

				return (
					<button
						key={info.provider}
						type="button"
						className={classes}
						role="option"
						aria-selected={isSelected}
						aria-disabled={isDisabled}
						disabled={isDisabled}
						onClick={isDisabled ? undefined : () => onSelect(info.provider)}
						style={{ border: "none", background: "none", width: "100%" }}
					>
						<div className="liquidGlass-effect" />
						<div className="liquidGlass-tint" />
						<div className="liquidGlass-shine" />
						<div className="liquidGlass-text provider-row__content">
							<span className="provider-row__label">
								<span className="provider-row__name">
									{formatProviderName(info.provider)}
								</span>
								<span className="provider-row__separator" aria-hidden="true">
									·
								</span>
								<span className="provider-row__account">
									{info.accountLabel}
								</span>
							</span>
							<span className="provider-row__chevron" aria-hidden="true">
								›
							</span>
						</div>
					</button>
				);
			})}
		</div>
	);
}
