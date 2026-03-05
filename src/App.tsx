import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";
import "./App.css";
import { PopoverShell } from "./components/PopoverShell";
import { StackNavigator } from "./navigation/StackNavigator";
import { NavigationProvider } from "./navigation/stack-context";
import { HomeView } from "./views/PlaceholderView";

// ── App ────────────────────────────────────────────────────────────────────

function App() {
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

	const initialView = {
		id: "home",
		title: "Home",
		component: <HomeView />,
	};

	return (
		<NavigationProvider initialView={initialView}>
			<PopoverShell>
				<StackNavigator />
			</PopoverShell>
		</NavigationProvider>
	);
}

export default App;
