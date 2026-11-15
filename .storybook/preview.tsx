import { useEffect } from "react";
import type { Preview } from "@storybook/react-vite";
import "../src/ui/ui.css";
const preview: Preview = {
  globalTypes: {
    theme: {
      description: "Colour theme",
      toolbar: {
        icon: "circlehollow",
        items: ["system", "light", "dark"],
        dynamicTitle: true,
      },
    },
  },
  initialGlobals: { theme: "system" },
  parameters: {
    a11y: { test: "error" },
    backgrounds: { disable: true },
    options: { storySort: { order: ["Foundations", "Components", "Pages"] } },
  },
  decorators: [
    (Story, context) => {
      useEffect(() => {
        if (context.globals.theme === "system")
          delete document.documentElement.dataset.theme;
        else document.documentElement.dataset.theme = context.globals.theme;
        document.body.style.background = "var(--canvas)";
      }, [context.globals.theme]);
      return <Story />;
    },
  ],
};
export default preview;
