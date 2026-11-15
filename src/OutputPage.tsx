import { ClipboardPaste, Type } from "lucide-react";
import { settingsSchema } from "./settings";
import { SaveStatus, SettingsPending } from "./SettingsStatus";
import { Select, SettingsGroup, SettingsRow as Row, Toggle } from "./ui";
import { useSettings } from "./useSettings";

export default function OutputPage() {
  const { query, save, busy, error } = useSettings();
  if (!query.data) return <SettingsPending query={query} />;
  const settings = query.data;
  const copyOnly = settings.pasteMethod === "copyOnly";
  return (
    <>
      <SettingsGroup
        title="Delivery"
        icon={<ClipboardPaste size={14} aria-hidden="true" />}
      >
        <Row
          id="paste-method"
          label="After you dictate"
          help={
            copyOnly
              ? "Nothing is typed for you. Paste with ⌘V when you’re ready."
              : "Your words appear wherever you’re typing."
          }
        >
          <Select
            id="paste-method"
            aria-describedby="paste-method-help"
            value={settings.pasteMethod}
            disabled={busy}
            onChange={(e) =>
              void save({
                pasteMethod: settingsSchema.shape.pasteMethod.parse(
                  e.target.value,
                ),
              })
            }
          >
            <option value="paste">Paste into the app</option>
            <option value="copyOnly">Copy to the clipboard</option>
          </Select>
        </Row>
        <Row
          id="restore-clipboard"
          label="Restore clipboard"
          help={
            copyOnly
              ? "Off while copying only: your words stay on the clipboard."
              : "Put back what you’d copied once your words are pasted."
          }
        >
          <Toggle
            id="restore-clipboard"
            aria-describedby="restore-clipboard-help"
            checked={settings.restoreClipboard && !copyOnly}
            disabled={busy || copyOnly}
            onChange={(e) => void save({ restoreClipboard: e.target.checked })}
          />
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Formatting"
        icon={<Type size={14} aria-hidden="true" />}
      >
        <Row
          id="capitalize-first"
          label="Capitalise the first letter"
          help="Start every dictation with a capital."
        >
          <Toggle
            id="capitalize-first"
            aria-describedby="capitalize-first-help"
            checked={settings.capitalizeFirst}
            disabled={busy}
            onChange={(e) => void save({ capitalizeFirst: e.target.checked })}
          />
        </Row>
        <Row
          id="trailing-space"
          label="Add a trailing space"
          help="Leaves room to keep typing or dictate the next sentence."
        >
          <Toggle
            id="trailing-space"
            aria-describedby="trailing-space-help"
            checked={settings.trailingSpace}
            disabled={busy}
            onChange={(e) => void save({ trailingSpace: e.target.checked })}
          />
        </Row>
      </SettingsGroup>
      <SaveStatus busy={busy} error={error} />
    </>
  );
}
