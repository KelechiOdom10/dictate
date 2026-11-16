import { AppWindow } from "lucide-react";
import { Switch } from "@astryxdesign/core/Switch";
import { AppearancePicker } from "./AppearancePicker";
import { Text } from "@astryxdesign/core/Text";
import { ErrorNotice, SettingsPending } from "./SettingsStatus";
import { SettingsGroup, SettingsRow as Row } from "./ui";
import { useSettings } from "./useSettings";
import { useDictationStatus } from "./useDictationStatus";

export default function AppearancePage() {
  const { query, save, busy, error } = useSettings();
  const { phase } = useDictationStatus();
  const blocked =
    phase === "annotating" || phase === "packaging"
      ? "Stop Annotate before changing appearance."
      : phase === "recording" || phase === "transcribing"
        ? "Finish dictating before changing appearance."
        : undefined;
  if (!query.data) return <SettingsPending query={query} />;
  return (
    <>
      <AppearancePicker
        themeLight={query.data.themeLight}
        themeDark={query.data.themeDark}
        mode={query.data.appearanceMode}
        disabledReason={blocked}
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
            isDisabled={!!blocked}
            disabledMessage={blocked}
            onChange={(checked) => void save({ glass: checked })}
          />
        </Row>
      </SettingsGroup>
      <ErrorNotice error={error} />
      <Text role="status" type="supporting">
        {blocked ?? (busy ? "Saving…" : "Changes save automatically.")}
      </Text>
    </>
  );
}
