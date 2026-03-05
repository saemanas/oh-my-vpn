import { GlassButton } from "../components/GlassButton";
import { useNavigation } from "../navigation/stack-context";
import type { ProviderInfo } from "../types/ipc";
import { DisconnectedView } from "./DisconnectedView";
import { ProviderSelection } from "./ProviderSelection";
import "./SuccessScreen.css";

// ── Types ──────────────────────────────────────────────────────────────────

type SuccessScreenProps = {
	/** The provider that was just successfully registered. */
	providerInfo: ProviderInfo;
};

// ── Helpers ────────────────────────────────────────────────────────────────

const PROVIDER_NAMES: Record<string, string> = {
	hetzner: "Hetzner",
	aws: "AWS",
	gcp: "GCP",
};

// ── Component ──────────────────────────────────────────────────────────────

/**
 * SuccessScreen -- shown after a provider is successfully registered.
 *
 * Displays a confirmation with the provider name and offers two actions:
 *   - "Add another provider" → pops back to ProviderSelection
 *   - "Done" → resets the navigation stack to DisconnectedView
 */
export function SuccessScreen({ providerInfo }: SuccessScreenProps) {
	const { push, reset } = useNavigation();

	const providerName =
		PROVIDER_NAMES[providerInfo.provider] ?? providerInfo.provider;

	function handleAddAnother() {
		push("provider-selection", "Add Provider", <ProviderSelection />);
	}

	function handleDone() {
		reset({
			id: "disconnected",
			title: "Disconnected",
			component: <DisconnectedView />,
		});
	}

	return (
		<div className="success-screen">
			<div className="success-screen__icon" aria-hidden="true">
				✅
			</div>
			<h2 className="success-screen__title">Provider Connected</h2>
			<p className="success-screen__message">
				{providerName} has been added successfully. You can add more providers
				or start using the app.
			</p>
			<div className="success-screen__actions">
				<GlassButton variant="neutral" onClick={handleAddAnother}>
					Add Another Provider
				</GlassButton>
				<GlassButton variant="success" onClick={handleDone}>
					Done
				</GlassButton>
			</div>
		</div>
	);
}
