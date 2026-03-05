import { useNavigation } from "../navigation/stack-context";
import type { Provider } from "../types/ipc";
import { ApiKeyInput } from "./ApiKeyInput";
import "./ProviderSelection.css";

// ── Provider metadata ──────────────────────────────────────────────────────

type ProviderOption = {
	provider: Provider;
	label: string;
	description: string;
};

const PROVIDERS: ProviderOption[] = [
	{
		provider: "hetzner",
		label: "Hetzner",
		description: "European cloud, affordable pricing",
	},
	{
		provider: "aws",
		label: "AWS",
		description: "Global coverage, 30+ regions",
	},
	{
		provider: "gcp",
		label: "GCP",
		description: "Google infrastructure, worldwide",
	},
];

// ── Component ──────────────────────────────────────────────────────────────

/**
 * ProviderSelection -- onboarding step where user picks a cloud provider.
 *
 * Renders three Liquid Glass cards (Hetzner, AWS, GCP). Tapping a card
 * pushes ApiKeyInput with the selected provider as a prop.
 */
export function ProviderSelection() {
	const { push } = useNavigation();

	function handleSelect(provider: Provider) {
		const option = PROVIDERS.find((p) => p.provider === provider);
		const title = option ? option.label : provider;
		push(`api-key-${provider}`, title, <ApiKeyInput provider={provider} />);
	}

	return (
		<div className="provider-selection">
			<p className="provider-selection__heading">Choose a cloud provider</p>
			<div className="provider-selection__list">
				{PROVIDERS.map((option) => (
					<button
						key={option.provider}
						type="button"
						className="liquidGlass-wrapper provider-card"
						onClick={() => handleSelect(option.provider)}
						style={{ border: "none", background: "none", width: "100%" }}
					>
						<div className="liquidGlass-effect" />
						<div className="liquidGlass-tint" />
						<div className="liquidGlass-shine" />
						<div className="liquidGlass-text provider-card__content">
							<span className="provider-card__label">{option.label}</span>
							<span className="provider-card__description">
								{option.description}
							</span>
						</div>
					</button>
				))}
			</div>
		</div>
	);
}
