import { useEffect } from "react";
import type { Preview } from "@storybook/react-vite";
import "../src/ui/ui.css";
import { SettingsTheme } from "../src/ui/SettingsTheme";
const preview: Preview = {
  globalTypes: {
    theme: {
      description: "Theme",
      toolbar: {
        icon: "paintbrush",
        items: [
          { value: "ember", title: "Ember" },
          { value: "paper", title: "Paper" },
        ],
        dynamicTitle: true,
      },
    },
    mode: {
      description: "Mode",
      toolbar: {
        icon: "circlehollow",
        items: ["light", "dark"],
        dynamicTitle: true,
      },
    },
  },
  initialGlobals: { theme: "ember", mode: "light" },
  parameters: {
    a11y: { test: "error" },
    backgrounds: { disable: true },
    options: { storySort: { order: ["Foundations", "Components", "Pages"] } },
  },
  decorators: [
    (Story, context) => {
      useEffect(() => {
        document.documentElement.dataset.palette = context.globals.theme;
        document.documentElement.dataset.mode = context.globals.mode;
        document.body.style.background = "var(--canvas)";
      }, [context.globals.theme, context.globals.mode]);
      if (
        [
          "Components/Button",
          "Components/Controls",
          "Components/Settings",
          "Components/ShortcutRecorder",
          "Components/Content",
        ].includes(context.title)
      )
        return (
          <SettingsTheme
            palette={context.globals.theme}
            mode={context.globals.mode}
          >
            <Story />
          </SettingsTheme>
        );
      return <Story />;
    },
  ],
};
export default preview;
