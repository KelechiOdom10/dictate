import { useId } from "react";
import { Card } from "@astryxdesign/core/Card";
import { RadioList, RadioListItem } from "@astryxdesign/core/RadioList";
import { Text } from "@astryxdesign/core/Text";
import { resolveThemeTokens } from "@astryxdesign/core/theme";
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

const grid = {
  display: "grid",
  gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
  gap: "var(--spacing-2)",
  maxWidth: "calc(var(--spacing-10) * 9)",
  width: "100%",
};

function Window({ id, side }: { id: ThemeId; side: Side }) {
  const color = resolveThemeTokens(themeFor(id), { mode: side });
  return (
    <>
      <rect
        x="0.5"
        y="0.5"
        width="119"
        height="71"
        rx="6"
        fill={color["--color-background-body"]}
        stroke={color["--color-border"]}
      />
      <rect
        x="1"
        y="1"
        width="34"
        height="70"
        rx="5.5"
        fill={color["--color-background-muted"]}
      />
      <rect
        x="6"
        y="10"
        width="24"
        height="7"
        rx="2"
        fill={color["--color-accent-muted"]}
      />
      <rect
        x="6"
        y="22"
        width="18"
        height="3"
        rx="1.5"
        fill={color["--color-text-secondary"]}
      />
      <rect
        x="6"
        y="30"
        width="21"
        height="3"
        rx="1.5"
        fill={color["--color-text-secondary"]}
      />
      <rect
        x="42"
        y="9"
        width="70"
        height="54"
        rx="4"
        fill={color["--color-background-card"]}
        stroke={color["--color-border"]}
      />
      <rect
        x="49"
        y="17"
        width="38"
        height="4"
        rx="2"
        fill={color["--color-text-primary"]}
      />
      <rect
        x="49"
        y="26"
        width="52"
        height="3"
        rx="1.5"
        fill={color["--color-text-secondary"]}
      />
      <rect
        x="49"
        y="32"
        width="44"
        height="3"
        rx="1.5"
        fill={color["--color-text-secondary"]}
      />
      <rect
        x="49"
        y="48"
        width="24"
        height="8"
        rx="4"
        fill={color["--color-accent"]}
      />
    </>
  );
}

/** A small picture of the window, split down the middle when it shows two sides. */
function Preview({ light, dark }: { light?: ThemeId; dark?: ThemeId }) {
  const clip = useId();
  return (
    <svg
      viewBox="0 0 120 72"
      width="100%"
      style={{ maxWidth: "calc(var(--spacing-10) * 2)" }}
      aria-hidden="true"
      display="block"
    >
      {light && <Window id={light} side="light" />}
      {dark && (
        <>
          <clipPath id={clip}>
            <rect x={light ? 60 : 0} y="0" width="120" height="72" />
          </clipPath>
          <g clipPath={`url(#${clip})`}>
            <Window id={dark} side="dark" />
          </g>
        </>
      )}
    </svg>
  );
}

function Tile({
  value,
  name,
  light,
  dark,
}: {
  value: string;
  name: string;
  light?: ThemeId;
  dark?: ThemeId;
}) {
  return (
    <RadioListItem
      value={value}
      align="start"
      label={
        <VStack gap={1}>
          <Preview light={light} dark={dark} />
          <Text type="supporting">{name}</Text>
        </VStack>
      }
    />
  );
}

export function AppearancePicker({
  themeLight,
  themeDark,
  mode,
  disabledReason,
  onChange,
}: {
  themeLight: ThemeId;
  themeDark: ThemeId;
  mode: AppearanceMode;
  disabledReason?: string;
  onChange: (patch: Partial<Settings>) => void;
}) {
  const systemDark = useSystemDark();
  const editing: Side = shownDark(mode, systemDark) ? "dark" : "light";
  const other: Side = editing === "dark" ? "light" : "dark";
  const themes = { light: themeLight, dark: themeDark };
  const sideName = { light: "Light", dark: "Dark" };
  return (
    <Card>
      <VStack gap={6}>
        <VStack gap={2}>
          <VStack gap={1}>
            <Text weight="medium">Mode</Text>
            <Text type="supporting">Choose a mode, then its theme below.</Text>
          </VStack>
          <RadioList
            label="Mode"
            isLabelHidden
            orientation="horizontal"
            value={mode}
            isDisabled={!!disabledReason}
            disabledMessage={disabledReason}
            size="sm"
            controlStyle={grid}
            onChange={(value) =>
              onChange({
                appearanceMode:
                  settingsSchema.shape.appearanceMode.parse(value),
              })
            }
          >
            <Tile
              value="system"
              name="System"
              light={themeLight}
              dark={themeDark}
            />
            <Tile value="light" name="Light" light={themeLight} />
            <Tile value="dark" name="Dark" dark={themeDark} />
          </RadioList>
        </VStack>
        <VStack gap={2}>
          <VStack gap={1}>
            <Text weight="medium">{sideName[editing]} theme</Text>
            <Text type="supporting">
              {mode === "system" ? `Your Mac is ${editing} now. ` : ""}
              {sideName[other]} uses {nameOf(themes[other])}.
            </Text>
          </VStack>
          <RadioList
            label={`${sideName[editing]} theme`}
            isLabelHidden
            orientation="horizontal"
            value={themes[editing]}
            isDisabled={!!disabledReason}
            disabledMessage={disabledReason}
            size="sm"
            controlStyle={grid}
            onChange={(value) => {
              const id = settingsSchema.shape.themeLight.parse(value);
              onChange(
                editing === "dark" ? { themeDark: id } : { themeLight: id },
              );
            }}
          >
            {THEMES.map((theme) => (
              <Tile
                key={theme.id}
                value={theme.id}
                name={theme.name}
                light={theme.id}
                dark={theme.id}
              />
            ))}
          </RadioList>
        </VStack>
      </VStack>
    </Card>
  );
}
