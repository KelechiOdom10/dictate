import { Bug, Mic, PanelBottom, Volume2 } from "lucide-react";
import { settingsSchema } from "./settings";
import { SaveStatus, SettingsPending } from "./SettingsStatus";
import { Select, SettingsGroup, SettingsRow as Row, Toggle } from "./ui";
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
          id="overlay-position"
          label="Overlay position"
          help="Where the recording overlay appears on your screen."
        >
          <Select
            id="overlay-position"
            aria-describedby="overlay-position-help"
            value={settings.overlayPosition}
            disabled={busy}
            onChange={(e) =>
              void save({
                overlayPosition: settingsSchema.shape.overlayPosition.parse(
                  e.target.value,
                ),
              })
            }
          >
            <option value="top">Top</option>
            <option value="bottom">Bottom</option>
          </Select>
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Microphone"
        icon={<Mic size={14} aria-hidden="true" />}
      >
        <Row
          id="mic-warm"
          label="Keep the microphone ready"
          help={
            settings.micMode === "warm"
              ? "Keeps the moment before you press the shortcut, so your first word isn’t clipped. macOS shows the microphone indicator the whole time."
              : "The microphone opens only while you dictate, so the first moment can be clipped."
          }
        >
          <Toggle
            id="mic-warm"
            aria-describedby="mic-warm-help"
            checked={settings.micMode === "warm"}
            disabled={busy}
            onChange={(e) =>
              void save({ micMode: e.target.checked ? "warm" : "onDemand" })
            }
          />
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Feedback"
        icon={<Volume2 size={14} aria-hidden="true" />}
      >
        <Row
          id="sound-feedback"
          label="Sound feedback"
          help="A soft sound when recording starts and stops."
        >
          <Toggle
            id="sound-feedback"
            aria-describedby="sound-feedback-help"
            checked={settings.soundFeedback}
            disabled={busy}
            onChange={(e) => void save({ soundFeedback: e.target.checked })}
          />
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Troubleshooting"
        icon={<Bug size={14} aria-hidden="true" />}
      >
        <Row
          id="debug-recordings"
          label="Save debug recordings"
          help="Keeps your last 20 recordings as audio files in the app data folder."
        >
          <Toggle
            id="debug-recordings"
            aria-describedby="debug-recordings-help"
            checked={settings.debugRecordings}
            disabled={busy}
            onChange={(e) => void save({ debugRecordings: e.target.checked })}
          />
        </Row>
      </SettingsGroup>
      <SaveStatus busy={busy} error={error} />
    </>
  );
}
