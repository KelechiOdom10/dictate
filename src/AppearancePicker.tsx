import { useSyncExternalStore } from "react";
import { SunMoon } from "lucide-react";
import { Badge } from "@astryxdesign/core/Badge";
import { Card } from "@astryxdesign/core/Card";
import { Heading } from "@astryxdesign/core/Heading";
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
            <StatusDot variant="accent" label="Accent" />
            <Text type="label">Aa</Text>
          </HStack>
          <Text type="supporting">Ready</Text>
          <HStack gap={1}>
            <Badge label="Pasted" />
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
    <VStack gap={2}>
      <HStack gap={2} vAlign="center">
        <SunMoon size={14} aria-hidden="true" />
        <Heading level={2}>Appearance</Heading>
      </HStack>
      <Card>
        <VStack gap={4}>
          <HStack gap={4} hAlign="between" vAlign="center">
            <VStack gap={0}>
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
            description="Each theme has a light and a dark side."
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
    </VStack>
  );
}
