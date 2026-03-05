import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import "./App.css";
import { PopoverShell } from "./components/PopoverShell";
import { StackNavigator } from "./navigation/StackNavigator";
import {
	NavigationProvider,
	useNavigation,
	type ViewEntry,
} from "./navigation/stack-context";
import type { ProviderInfo, SessionStatus } from "./types/ipc";
import { ConnectedView } from "./views/ConnectedView";
import { DisconnectedView } from "./views/DisconnectedView";
import { ProviderManagementView } from "./views/ProviderManagementView";
import { SettingsView } from "./views/SettingsView";
import { SystemPermissionsView } from "./views/SystemPermissionsView";
import { WelcomeScreen } from "./views/WelcomeScreen";

// ── Navigate Listener ──────────────────────────────────────────────────────

/** View ID → component mapping for tray context menu navigation events. */
const NAVIGATE_VIEWS: Record<string, { title: string; component: React.ReactNode }> = {
	"provider-management": { title: "Provider Management", component: <ProviderManagementView /> },
	"system-permissions": { title: "System Permissions", component: <SystemPermissionsView /> },
	settings: { title: "Settings", component: <SettingsView /> },
};

/**
 * Listens for "navigate" events emitted by the tray context menu and pushes
 * the corresponding view onto the navigation stack. Must be rendered inside
 * NavigationProvider.
 */
function NavigateListener() {
	const { push } = useNavigation();

	useEffect(() => {
		const unlisten = listen<string>("navigate", (event) => {
			const view = NAVIGATE_VIEWS[event.payload];
			if (view) {
				push(event.payload, view.title, view.component);
			}
		});
		return () => {
			void unlisten.then((fn) => fn());
		};
	}, [push]);

	return null;
}

// ── App ────────────────────────────────────────────────────────────────────

function App() {
	const [initialView, setInitialView] = useState<ViewEntry | null>(null);

	// Esc key: hide the popover window
	useEffect(() => {
		function handleKeyDown(event: KeyboardEvent) {
			if (event.key === "Escape") {
				void getCurrentWindow().hide();
			}
		}
		document.addEventListener("keydown", handleKeyDown);
		return () => document.removeEventListener("keydown", handleKeyDown);
	}, []);

	// Session check on mount: determine initial view
	useEffect(() => {
		async function checkSession() {
			try {
				const status = await invoke<SessionStatus | null>("get_session_status");
				if (status) {
					setInitialView({
						id: "connected",
						title: "Connected",
						component: <ConnectedView initialSession={status} />,
					});
					return;
				}
			} catch {
				// Error fetching session -- fall through to provider check
			}

			// First-run detection: no providers registered → onboarding
			try {
				const providers = await invoke<ProviderInfo[]>("list_providers");
				if (providers.length === 0) {
					setInitialView({
						id: "welcome",
						title: "Welcome",
						component: <WelcomeScreen />,
					});
					return;
				}
			} catch {
				// Error fetching providers -- fall through to disconnected
			}

			setInitialView({
				id: "disconnected",
				title: "Disconnected",
				component: <DisconnectedView />,
			});
		}
		void checkSession();
	}, []);

	// Brief loading while session check completes -- no flash
	if (!initialView) {
		return (
			<PopoverShell>
				<div />
			</PopoverShell>
		);
	}

	return (
		<NavigationProvider initialView={initialView}>
			<NavigateListener />
			<PopoverShell>
				<StackNavigator />
			</PopoverShell>
		</NavigationProvider>
	);
}

export default App;
