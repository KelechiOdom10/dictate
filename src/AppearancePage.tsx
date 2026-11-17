import { AppearancePicker } from "./AppearancePicker";
import { Text } from "@astryxdesign/core/Text";
import { ErrorNotice, SettingsPending } from "./SettingsStatus";
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
      <ErrorNotice error={error} />
      <Text role="status" type="supporting">
        {blocked ?? (busy ? "Saving…" : "Changes save automatically.")}
      </Text>
    </>
  );
}
