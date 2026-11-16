import { AppearancePicker } from "./AppearancePicker";
import { SaveStatus, SettingsPending } from "./SettingsStatus";
import { useSettings } from "./useSettings";

export default function AppearancePage() {
  const { query, save, busy, error } = useSettings();
  if (!query.data) return <SettingsPending query={query} />;
  return (
    <>
      <AppearancePicker
        theme={query.data.themeLight}
        mode={query.data.appearanceMode}
        disabled={busy}
        onChange={(patch) => void save(patch)}
      />
      <SaveStatus busy={busy} error={error} />
    </>
  );
}
