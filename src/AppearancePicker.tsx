import { useState } from "react";
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
  shownDark,
  themeFor,
  useSystemDark,
  type AppearanceMode,
  type ThemeId,
} from "./ui/SettingsTheme";

type Side = "light" | "dark";

const nameOf = (id: ThemeId) =>
  THEMES.find((theme) => theme.id === id)?.name ?? "Ember";

function Preview({ id, mode }: { id: ThemeId; mode: Side }) {
  return (
    <Theme theme={themeFor(id)} mode={mode}>
      <Card padding={1} width={112}>
        <VStack gap={1}>
          <HStack gap={1} vAlign="center">
            <Text type="label">Aa</Text>
            <StatusDot variant="accent" label="Accent" />
          </HStack>
          <HStack gap={1} vAlign="center">
            <StatusDot variant="success" label="Positive" />
            <StatusDot variant="warning" label="Caution" />
            <Text type="supporting">Body</Text>
          </HStack>
        </VStack>
      </Card>
    </Theme>
  );
}

export function AppearancePicker({
  themeLight,
  themeDark,
  mode,
  disabled,
  onChange,
}: {
  themeLight: ThemeId;
  themeDark: ThemeId;
  mode: AppearanceMode;
  disabled: boolean;
  onChange: (patch: Partial<Settings>) => void;
}) {
  const dark = shownDark(mode, useSystemDark());
  // The side being edited starts as the one on screen and then stays put, so
  // the Mac flipping its appearance never moves the list under your hands.
  const [side, setSide] = useState<Side>(() => (dark ? "dark" : "light"));
  const theme = side === "dark" ? themeDark : themeLight;
  return (
    <Card>
      <VStack gap={6}>
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
        <VStack gap={3}>
          <VStack gap={2}>
            <HStack gap={2} vAlign="center">
              <Text weight="medium">Theme</Text>
              {(side === "dark") === dark && (
                <HStack gap={1} vAlign="center">
                  <StatusDot variant="accent" label="Current appearance" />
                  <Text type="supporting">In use now</Text>
                </HStack>
              )}
            </HStack>
            <Text type="supporting">
              Light uses {nameOf(themeLight)} · Dark uses {nameOf(themeDark)}
            </Text>
            <SegmentedControl
              label="Theme for"
              value={side}
              isDisabled={disabled}
              onChange={(value) => setSide(value as Side)}
            >
              <SegmentedControlItem value="light" label="Light theme" />
              <SegmentedControlItem value="dark" label="Dark theme" />
            </SegmentedControl>
          </VStack>
          <RadioList
            label={`Theme for ${side}`}
            value={theme}
            isDisabled={disabled}
            onChange={(value) => {
              const id = settingsSchema.shape.themeLight.parse(value);
              onChange(
                side === "dark" ? { themeDark: id } : { themeLight: id },
              );
            }}
          >
            {THEMES.map((option) => (
              <RadioListItem
                key={option.id}
                value={option.id}
                label={option.name}
                description={option.description}
                endContent={<Preview id={option.id} mode={side} />}
              />
            ))}
          </RadioList>
        </VStack>
      </VStack>
    </Card>
  );
}
