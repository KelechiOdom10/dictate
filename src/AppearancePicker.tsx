import { useSyncExternalStore } from "react";
import { Card } from "@astryxdesign/core/Card";
import { HStack } from "@astryxdesign/core/HStack";
import { RadioList, RadioListItem } from "@astryxdesign/core/RadioList";
import {
  SegmentedControl,
  SegmentedControlItem,
} from "@astryxdesign/core/SegmentedControl";
import { StatusDot } from "@astryxdesign/core/StatusDot";
import { Text } from "@astryxdesign/core/Text";
import { Theme } from "@astryxdesign/core/theme";
import { VStack } from "@astryxdesign/core/VStack";
import { settingsSchema, type Settings } from "./settings";
import {
  THEMES,
  themeFor,
  type AppearanceMode,
  type ThemeId,
} from "./ui/SettingsTheme";

const darkQuery = "(prefers-color-scheme: dark)";
function useSystemDark() {
  return useSyncExternalStore(
    (change) => {
      const query = matchMedia(darkQuery);
      query.addEventListener("change", change);
      return () => query.removeEventListener("change", change);
    },
    () => matchMedia(darkQuery).matches,
  );
}

function Preview({ id, mode }: { id: ThemeId; mode: "light" | "dark" }) {
  return (
    <Theme theme={themeFor(id)} mode={mode}>
      <Card padding={2} width={112}>
        <VStack gap={1}>
          <HStack gap={1} vAlign="center">
            <Text type="label">Aa</Text>
            <StatusDot variant="accent" label="Accent" />
          </HStack>
          <HStack gap={1} vAlign="center">
            <StatusDot variant="success" label="Positive" />
            <StatusDot variant="warning" label="Caution" />
            <Text type="supporting">Body text</Text>
          </HStack>
        </VStack>
      </Card>
    </Theme>
  );
}

export function AppearancePicker({
  theme,
  mode,
  disabled,
  onChange,
}: {
  theme: ThemeId;
  mode: AppearanceMode;
  disabled: boolean;
  onChange: (patch: Partial<Settings>) => void;
}) {
  const systemDark = useSystemDark();
  const shown = mode === "system" ? (systemDark ? "dark" : "light") : mode;
  return (
    <Card>
      <VStack gap={4}>
        <HStack gap={4} hAlign="between" vAlign="center">
          <VStack gap={1}>
            <Text weight="medium">Mode</Text>
            <Text type="supporting">Follow your Mac, or choose a look.</Text>
          </VStack>
          <SegmentedControl
            label="Mode"
            value={mode}
            isDisabled={disabled}
            onChange={(value) =>
              onChange({
                appearanceMode:
                  settingsSchema.shape.appearanceMode.parse(value),
              })
            }
          >
            <SegmentedControlItem value="system" label="System" />
            <SegmentedControlItem value="light" label="Light" />
            <SegmentedControlItem value="dark" label="Dark" />
          </SegmentedControl>
        </HStack>
        <RadioList
          label="Theme"
          value={theme}
          isDisabled={disabled}
          onChange={(value) =>
            onChange({ theme: settingsSchema.shape.theme.parse(value) })
          }
        >
          {THEMES.map((option) => (
            <RadioListItem
              key={option.id}
              value={option.id}
              label={option.name}
              description={option.description}
              endContent={<Preview id={option.id} mode={shown} />}
            />
          ))}
        </RadioList>
      </VStack>
    </Card>
  );
}
