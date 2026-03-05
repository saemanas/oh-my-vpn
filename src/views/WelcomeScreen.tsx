import { GlassButton } from "../components/GlassButton";
import { useNavigation } from "../navigation/stack-context";
import { ProviderSelection } from "./ProviderSelection";
import "./WelcomeScreen.css";

// ── Component ──────────────────────────────────────────────────────────────

/**
 * WelcomeScreen -- shown on first launch when no providers are registered.
 *
 * Displays a lock icon, app title, subtitle explaining the app purpose,
 * and a "Get Started" button that pushes ProviderSelection.
 */
export function WelcomeScreen() {
	const { push } = useNavigation();

	function handleGetStarted() {
		push("provider-selection", "Add Provider", <ProviderSelection />);
	}

	return (
		<div className="welcome-screen">
			<div className="welcome-screen__icon" aria-hidden="true">
				🔒
			</div>
			<h1 className="welcome-screen__title">Oh My VPN</h1>
			<p className="welcome-screen__subtitle">
				Create a private VPN server in seconds. Your traffic, your server, your
				control.
			</p>
			<GlassButton variant="success" onClick={handleGetStarted}>
				Get Started
			</GlassButton>
		</div>
	);
}
