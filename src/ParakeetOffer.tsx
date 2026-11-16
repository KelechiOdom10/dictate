import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Sparkles } from "lucide-react";
import { downloadModel, formatBytes, readModels } from "./models";
import { readSettings, writeSettings } from "./settings";
import { Button } from "./ui";

const PARAKEET = "parakeet-unified-en";

/**
 * Asks people who were on a Whisper model before Parakeet became the
 * default whether to download it. Nothing downloads until they say so, and
 * either answer clears `offerParakeet` so it never asks again.
 */
export default function ParakeetOffer({
  onDownload,
}: {
  onDownload: () => void;
}) {
  const client = useQueryClient();
  const settings = useQuery({ queryKey: ["settings"], queryFn: readSettings });
  const models = useQuery({ queryKey: ["models"], queryFn: readModels });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const parakeet = models.data?.find((model) => model.id === PARAKEET);
  const current = models.data?.find(
    (model) => model.id === settings.data?.speechModel,
  );
  if (
    !settings.data?.offerParakeet ||
    settings.data.speechModel === PARAKEET ||
    parakeet?.state.kind !== "available"
  )
    return null;

  async function answer(download: boolean) {
    if (!settings.data) return;
    setBusy(true);
    setError("");
    try {
      // Download first: clearing the flag hides this prompt, and with it
      // any error the download would report.
      if (download) await downloadModel(PARAKEET);
      const saved = await writeSettings({
        ...settings.data,
        offerParakeet: false,
      });
      client.setQueryData(["settings"], saved);
      if (download) {
        await client.invalidateQueries({ queryKey: ["models"] });
        onDownload();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const size = formatBytes(parakeet.bytes);
  return (
    <div
      className="ui-toast ui-toast-info parakeet-offer"
      role="region"
      aria-labelledby="parakeet-offer-title"
    >
      <span aria-hidden="true">
        <Sparkles size={16} />
      </span>
      <div className="parakeet-offer-body">
        <p id="parakeet-offer-title" className="parakeet-offer-title">
          Try {parakeet.name}, now the recommended model
        </p>
        <p>
          It’s faster than {current?.name ?? "your current model"} and stays
          loaded, so your words appear sooner. You’ll keep using{" "}
          {current?.name ?? "your current model"} until you switch.
        </p>
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        <div className="ui-actions">
          <Button
            size="sm"
            variant="primary"
            disabled={busy}
            onClick={() => void answer(true)}
          >
            Download ({size})
          </Button>
          <Button size="sm" disabled={busy} onClick={() => void answer(false)}>
            Not now
          </Button>
        </div>
      </div>
    </div>
  );
}
