import { ClipboardPaste, Type } from "lucide-react";
import { Selector } from "@astryxdesign/core/Selector";
import { Switch } from "@astryxdesign/core/Switch";
import { settingsSchema } from "./settings";
import { SaveStatus, SettingsPending } from "./SettingsStatus";
import { SettingsGroup, SettingsRow as Row } from "./ui";
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
          label="After dictation"
          help={
            copyOnly
              ? "Nothing is typed for you. Paste with ⌘V when you’re ready."
              : "Your words appear wherever you’re typing."
          }
        >
          <Selector
            label="After dictation"
            isLabelHidden
            value={
              copyOnly
                ? "copyOnly"
                : settings.restoreClipboard
                  ? "paste"
                  : "pasteKeep"
            }
            isDisabled={busy}
            options={[
              { value: "paste", label: "Paste" },
              { value: "copyOnly", label: "Copy only" },
              { value: "pasteKeep", label: "Paste and keep on clipboard" },
            ]}
            onChange={(value) =>
              void save({
                pasteMethod: settingsSchema.shape.pasteMethod.parse(
                  value === "pasteKeep" ? "paste" : value,
                ),
                restoreClipboard: value === "paste",
              })
            }
          />
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Formatting"
        icon={<Type size={14} aria-hidden="true" />}
      >
        <Row
          label="Capitalise the first letter"
          help="Start every dictation with a capital."
        >
          <Switch
            label="Capitalise the first letter"
            isLabelHidden
            value={settings.capitalizeFirst}
            isDisabled={busy}
            onChange={(checked) => void save({ capitalizeFirst: checked })}
          />
        </Row>
        <Row
          label="Add a trailing space"
          help="Leaves room to keep typing or dictate the next sentence."
        >
          <Switch
            label="Add a trailing space"
            isLabelHidden
            value={settings.trailingSpace}
            isDisabled={busy}
            onChange={(checked) => void save({ trailingSpace: checked })}
          />
        </Row>
      </SettingsGroup>
      <SaveStatus busy={busy} error={error} />
    </>
  );
}
