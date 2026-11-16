import { Card } from "@astryxdesign/core/Card";
import { Grid } from "@astryxdesign/core/Grid";
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

function Preview({ id, mode }: { id: ThemeId; mode: Side }) {
  return (
    <Theme theme={themeFor(id)} mode={mode}>
      <Card padding={1} width={72}>
        <VStack gap={1}>
          <HStack gap={1} vAlign="center">
            <Text type="label">Aa</Text>
            <StatusDot variant="accent" label="Accent" />
          </HStack>
          <HStack gap={1} vAlign="center">
            <StatusDot variant="success" label="Positive" />
            <StatusDot variant="warning" label="Caution" />
          </HStack>
        </VStack>
      </Card>
    </Theme>
  );
}

function ThemeColumn({
  side,
  theme,
  disabled,
  isShown,
  onChange,
}: {
  side: Side;
  theme: ThemeId;
  disabled: boolean;
  isShown: boolean;
  onChange: (theme: ThemeId) => void;
}) {
  const name = side === "light" ? "Light" : "Dark";
  return (
    <VStack gap={2}>
      <VStack gap={1}>
        <HStack gap={2} vAlign="center">
          <Text weight="medium">{name}</Text>
          {isShown && (
            <HStack gap={1} vAlign="center">
              <StatusDot variant="accent" label="Current appearance" />
              <Text type="supporting">In use now</Text>
            </HStack>
          )}
        </HStack>
        <Text type="supporting">
          {side === "light"
            ? "Used when the window is light."
            : "Used when the window is dark."}
        </Text>
      </VStack>
      <RadioList
        label={`Theme in ${side}`}
        value={theme}
        isDisabled={disabled}
        onChange={(value) =>
          onChange(settingsSchema.shape.themeLight.parse(value))
        }
      >
        {THEMES.map((option) => (
          <RadioListItem
            key={option.id}
            value={option.id}
            label={option.name}
            endContent={<Preview id={option.id} mode={side} />}
          />
        ))}
      </RadioList>
    </VStack>
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
        <Grid columns={{ minWidth: 200, max: 2 }} gap={4}>
          <ThemeColumn
            side="light"
            theme={themeLight}
            disabled={disabled}
            isShown={!dark}
            onChange={(theme) => onChange({ themeLight: theme })}
          />
          <ThemeColumn
            side="dark"
            theme={themeDark}
            disabled={disabled}
            isShown={dark}
            onChange={(theme) => onChange({ themeDark: theme })}
          />
        </Grid>
      </VStack>
    </Card>
  );
}
