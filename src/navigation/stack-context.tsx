import type React from "react";
import { createContext, useCallback, useContext, useState } from "react";

// ── Types ──────────────────────────────────────────────────────────────────

export type ViewEntry = {
	id: string;
	title: string;
	component: React.ReactNode;
};

export type TransitionDirection = "push" | "pop";

export type NavigationContextType = {
	stack: ViewEntry[];
	push: (id: string, title: string, component: React.ReactNode) => void;
	pop: () => void;
	/** Replace the entire stack with a single view, animating as a push. */
	reset: (view: ViewEntry) => void;
	canGoBack: boolean;
	currentView: ViewEntry | null;
	previousTitle: string | null;
	direction: TransitionDirection;
	isTransitioning: boolean;
	onTransitionEnd: () => void;
};

// ── Context ────────────────────────────────────────────────────────────────

const NavigationContext = createContext<NavigationContextType | null>(null);

// ── Provider ───────────────────────────────────────────────────────────────

type NavigationProviderProps = {
	initialView: ViewEntry;
	children: React.ReactNode;
};

export function NavigationProvider({
	initialView,
	children,
}: NavigationProviderProps) {
	const [stack, setStack] = useState<ViewEntry[]>([initialView]);
	const [direction, setDirection] = useState<TransitionDirection>("push");
	const [isTransitioning, setIsTransitioning] = useState(false);

	const push = useCallback(
		(id: string, title: string, component: React.ReactNode) => {
			if (isTransitioning) return;
			setDirection("push");
			setIsTransitioning(true);
			setStack((prev) => [...prev, { id, title, component }]);
		},
		[isTransitioning],
	);

	const pop = useCallback(() => {
		if (isTransitioning) return;
		setStack((prev) => {
			if (prev.length <= 1) return prev;
			setDirection("pop");
			setIsTransitioning(true);
			return prev;
		});
	}, [isTransitioning]);

	const reset = useCallback(
		(view: ViewEntry) => {
			if (isTransitioning) return;
			// Immediate replacement -- no animation. Clears the entire stack and
			// replaces it with a single view. Unlike push/pop, reset is a full
			// context switch (e.g. onboarding → main app) so animating between
			// unrelated views would be disorienting.
			setIsTransitioning(false);
			setDirection("push");
			setStack([view]);
		},
		[isTransitioning],
	);

	const onTransitionEnd = useCallback(() => {
		setIsTransitioning(false);
		setStack((prev) => {
			if (direction === "pop" && prev.length > 1) {
				return prev.slice(0, -1);
			}
			return prev;
		});
	}, [direction]);

	const canGoBack = stack.length > 1;
	const currentView = stack.length > 0 ? stack[stack.length - 1] : null;
	const previousTitle = stack.length > 1 ? stack[stack.length - 2].title : null;

	return (
		<NavigationContext.Provider
			value={{
				stack,
				push,
				pop,
				reset,
				canGoBack,
				currentView,
				previousTitle,
				direction,
				isTransitioning,
				onTransitionEnd,
			}}
		>
			{children}
		</NavigationContext.Provider>
	);
}

// ── Hook ───────────────────────────────────────────────────────────────────

export function useNavigation(): NavigationContextType {
	const context = useContext(NavigationContext);
	if (context === null) {
		throw new Error("useNavigation must be used within a NavigationProvider");
	}
	return context;
}
