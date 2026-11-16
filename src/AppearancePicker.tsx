import { Card } from "@astryxdesign/core/Card";
import { Grid } from "@astryxdesign/core/Grid";
import { HStack } from "@astryxdesign/core/HStack";
import { StatusDot } from "@astryxdesign/core/StatusDot";
import { Text } from "@astryxdesign/core/Text";
import { Theme } from "@astryxdesign/core/theme";
import { VStack } from "@astryxdesign/core/VStack";
import { settingsSchema, type Settings } from "./settings";
import { Choice } from "./ui/Choice";
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

/** The theme's own colours, so a tile shows what it will look like. */
function Swatch({
  id,
  mode,
  compact = false,
}: {
  id: ThemeId;
  mode: Side;
  compact?: boolean;
}) {
  return (
    <Theme theme={themeFor(id)} mode={mode}>
      <Card padding={compact ? 1 : 2} width="100%">
        <VStack gap={1}>
          <Text type="label">Aa</Text>
          <HStack gap={1} vAlign="center">
            <StatusDot variant="accent" label="Accent" />
            <StatusDot variant="success" label="Positive" />
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
  const setMode = (value: string) =>
    onChange({
      appearanceMode: settingsSchema.shape.appearanceMode.parse(value),
    });
  const setTheme = (side: Side) => (value: string) => {
    const id = settingsSchema.shape.themeLight.parse(value);
    onChange(side === "dark" ? { themeDark: id } : { themeLight: id });
  };
  return (
    <Card>
      <VStack gap={6}>
        <VStack gap={2}>
          <Text weight="medium">Mode</Text>
          <Text type="supporting">Follow your Mac, or choose a look.</Text>
          <HStack gap={2} role="radiogroup" aria-label="Mode">
            <Choice
              name="appearance-mode"
              value="system"
              label="System"
              checked={mode === "system"}
              disabled={disabled}
              onChange={() => setMode("system")}
            >
              <HStack gap={1} aria-hidden="true">
                <Swatch id={themeLight} mode="light" compact />
                <Swatch id={themeDark} mode="dark" compact />
              </HStack>
              <Text type="supporting">System</Text>
            </Choice>
            <Choice
              name="appearance-mode"
              value="light"
              label="Light"
              checked={mode === "light"}
              disabled={disabled}
              onChange={() => setMode("light")}
            >
              <Swatch id={themeLight} mode="light" compact />
              <Text type="supporting">Light</Text>
            </Choice>
            <Choice
              name="appearance-mode"
              value="dark"
              label="Dark"
              checked={mode === "dark"}
              disabled={disabled}
              onChange={() => setMode("dark")}
            >
              <Swatch id={themeDark} mode="dark" compact />
              <Text type="supporting">Dark</Text>
            </Choice>
          </HStack>
        </VStack>
        <VStack gap={2}>
          <Text weight="medium">Theme</Text>
          <Text type="supporting">
            Pick the light side and the dark side of each theme.
          </Text>
          <Grid columns={{ minWidth: 190, max: 2 }} gap={2}>
            {THEMES.map((option) => (
              <Card key={option.id} padding={2}>
                <VStack gap={2}>
                  <HStack gap={1}>
                    <Choice
                      name="theme-light"
                      value={option.id}
                      label={`Set light theme to ${option.name}`}
                      checked={themeLight === option.id}
                      disabled={disabled}
                      onChange={() => setTheme("light")(option.id)}
                    >
                      <VStack gap={1}>
                        <Swatch id={option.id} mode="light" />
                        <HStack gap={1} vAlign="center">
                          <Text type="supporting">Light</Text>
                          {!dark && themeLight === option.id && (
                            <StatusDot
                              variant="accent"
                              label="Current appearance"
                              tooltip="In use now"
                            />
                          )}
                        </HStack>
                      </VStack>
                    </Choice>
                    <Choice
                      name="theme-dark"
                      value={option.id}
                      label={`Set dark theme to ${option.name}`}
                      checked={themeDark === option.id}
                      disabled={disabled}
                      onChange={() => setTheme("dark")(option.id)}
                    >
                      <VStack gap={1}>
                        <Swatch id={option.id} mode="dark" />
                        <HStack gap={1} vAlign="center">
                          <Text type="supporting">Dark</Text>
                          {dark && themeDark === option.id && (
                            <StatusDot
                              variant="accent"
                              label="Current appearance"
                              tooltip="In use now"
                            />
                          )}
                        </HStack>
                      </VStack>
                    </Choice>
                  </HStack>
                  <Text weight="medium">{option.name}</Text>
                </VStack>
              </Card>
            ))}
          </Grid>
          <Text type="supporting">
            {themeLight === themeDark
              ? `${nameOf(themeLight)} in both`
              : `Light uses ${nameOf(themeLight)} · Dark uses ${nameOf(themeDark)}`}
          </Text>
        </VStack>
      </VStack>
    </Card>
  );
}
