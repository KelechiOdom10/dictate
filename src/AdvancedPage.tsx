import { Bug, Mic, PanelBottom, Volume2 } from "lucide-react";
import { Selector } from "@astryxdesign/core/Selector";
import { Switch } from "@astryxdesign/core/Switch";
import { settingsSchema } from "./settings";
import { SaveStatus, SettingsPending } from "./SettingsStatus";
import { SettingsGroup, SettingsRow as Row } from "./ui";
import { useSettings } from "./useSettings";

export default function AdvancedPage() {
  const { query, save, busy, error } = useSettings();
  if (!query.data) return <SettingsPending query={query} />;
  const settings = query.data;
  return (
    <>
      <SettingsGroup
        title="Overlay"
        icon={<PanelBottom size={14} aria-hidden="true" />}
      >
        <Row
          label="Overlay position"
          help="Where the recording overlay appears on your screen."
        >
          <Selector
            label="Overlay position"
            isLabelHidden
            value={settings.overlayPosition}
            isDisabled={busy}
            options={[
              { value: "top", label: "Top" },
              { value: "bottom", label: "Bottom" },
            ]}
            onChange={(value) =>
              void save({
                overlayPosition:
                  settingsSchema.shape.overlayPosition.parse(value),
              })
            }
          />
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Microphone"
        icon={<Mic size={14} aria-hidden="true" />}
      >
        <Row
          label="Keep the microphone ready"
          help={
            settings.micMode === "warm"
              ? "Keeps the moment before you press the shortcut, so your first word isn’t clipped. macOS shows the microphone indicator the whole time."
              : "The microphone opens only while you dictate, so the first moment can be clipped."
          }
        >
          <Switch
            label="Keep the microphone ready"
            isLabelHidden
            value={settings.micMode === "warm"}
            isDisabled={busy}
            onChange={(checked) =>
              void save({ micMode: checked ? "warm" : "onDemand" })
            }
          />
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Feedback"
        icon={<Volume2 size={14} aria-hidden="true" />}
      >
        <Row
          label="Sound feedback"
          help="A soft sound when recording starts and stops."
        >
          <Switch
            label="Sound feedback"
            isLabelHidden
            value={settings.soundFeedback}
            isDisabled={busy}
            onChange={(checked) => void save({ soundFeedback: checked })}
          />
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Troubleshooting"
        icon={<Bug size={14} aria-hidden="true" />}
      >
        <Row
          label="Save debug recordings"
          help="Keeps your last 20 recordings as audio files in the app data folder."
        >
          <Switch
            label="Save debug recordings"
            isLabelHidden
            value={settings.debugRecordings}
            isDisabled={busy}
            onChange={(checked) => void save({ debugRecordings: checked })}
          />
        </Row>
      </SettingsGroup>
      <SaveStatus busy={busy} error={error} />
    </>
  );
}
