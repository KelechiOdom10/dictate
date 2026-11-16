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
          label="After dictation"
          help={
            copyOnly
              ? "Nothing is typed for you. Paste with ⌘V when you’re ready."
              : "Your words appear wherever you’re typing."
          }
        >
          <Select
            id="paste-method"
            aria-describedby="paste-method-help"
            value={
              copyOnly
                ? "copyOnly"
                : settings.restoreClipboard
                  ? "paste"
                  : "pasteKeep"
            }
            disabled={busy}
            onChange={(e) =>
              void save({
                pasteMethod: settingsSchema.shape.pasteMethod.parse(
                  e.target.value === "pasteKeep" ? "paste" : e.target.value,
                ),
                restoreClipboard: e.target.value === "paste",
              })
            }
          >
            <option value="paste">Paste</option>
            <option value="copyOnly">Copy only</option>
            <option value="pasteKeep">Paste and keep on clipboard</option>
          </Select>
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
