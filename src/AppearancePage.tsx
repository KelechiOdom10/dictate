import { AppWindow } from "lucide-react";
import { Switch } from "@astryxdesign/core/Switch";
import { AppearancePicker } from "./AppearancePicker";
import { SaveStatus, SettingsPending } from "./SettingsStatus";
import { SettingsGroup, SettingsRow as Row } from "./ui";
import { useSettings } from "./useSettings";

export default function AppearancePage() {
  const { query, save, busy, error } = useSettings();
  if (!query.data) return <SettingsPending query={query} />;
  return (
    <>
      <AppearancePicker
        themeLight={query.data.themeLight}
        themeDark={query.data.themeDark}
        mode={query.data.appearanceMode}
        disabled={busy}
        onChange={(patch) => void save(patch)}
      />
      <SettingsGroup
        title="Window"
        icon={<AppWindow size={14} aria-hidden="true" />}
      >
        <Row
          label="Translucent window"
          help="Lets what is behind the window show through."
        >
          <Switch
            label="Translucent window"
            isLabelHidden
            value={query.data.glass}
            isDisabled={busy}
            onChange={(checked) => void save({ glass: checked })}
          />
        </Row>
      </SettingsGroup>
      <SaveStatus busy={busy} error={error} />
    </>
  );
}
