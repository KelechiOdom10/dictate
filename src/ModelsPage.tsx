import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CircleAlert } from "lucide-react";
import {
  accuracyLabel,
  cancelDownload,
  deleteModel,
  downloadModel,
  formatBytes,
  MEASURED_CLIP_SECONDS,
  readModels,
  readWhisperStatus,
  speedLabel,
  WHISPER_INSTALL,
  type ModelEntry,
  type SpeechModel,
} from "./models";
import { readSettings, writeSettings } from "./settings";
import { Button, ModelCard, type ModelStatus } from "./ui";

export default function ModelsPage() {
  const client = useQueryClient();
  const models = useQuery({ queryKey: ["models"], queryFn: readModels });
  const whisper = useQuery({
    queryKey: ["whisper"],
    queryFn: readWhisperStatus,
  });
  const settings = useQuery({ queryKey: ["settings"], queryFn: readSettings });
  const [error, setError] = useState("");
  const refresh = () => client.invalidateQueries({ queryKey: ["models"] });
  const action = useMutation({
    mutationFn: ({
      run,
      model,
    }: {
      run: (model: SpeechModel) => Promise<unknown>;
      model: SpeechModel;
    }) => run(model),
    onMutate: () => setError(""),
    onSettled: refresh,
    onError: (e) => setError(String(e)),
  });
  const select = useMutation({
    mutationFn: async (model: SpeechModel) => {
      if (!settings.data) throw new Error("Settings are still loading.");
      return writeSettings({ ...settings.data, speechModel: model });
    },
    onMutate: () => setError(""),
    onSuccess: (saved) => client.setQueryData(["settings"], saved),
    onSettled: refresh,
    onError: (e) => setError(String(e)),
  });

  useEffect(() => {
    const subscriptions = ["models-changed", "settings-changed"].map((event) =>
      listen(event, () => client.invalidateQueries({ queryKey: ["models"] })),
    );
    return () => {
      for (const subscription of subscriptions)
        void subscription.then((stop) => stop()).catch(console.error);
    };
  }, [client]);

  if (models.isPending)
    return (
      <div className="loading" role="status">
        Loading models…
      </div>
    );
  if (models.isError)
    return (
      <div className="notice" role="alert">
        <p>{String(models.error)}</p>
        <Button onClick={() => void models.refetch()}>Try again</Button>
      </div>
    );

  const fastest = Math.min(...models.data.map((model) => model.measuredMs));
  const busy = action.isPending || select.isPending;
  function status(model: ModelEntry): ModelStatus {
    const run = (run: (model: SpeechModel) => Promise<unknown>) => () =>
      action.mutate({ run, model: model.id });
    switch (model.state.kind) {
      case "available":
        return { kind: "available", onDownload: run(downloadModel) };
      case "downloading": {
        const { received, total, cancelling } = model.state;
        return {
          kind: "downloading",
          progress: (received / total) * 100,
          detail: `${formatBytes(received)} of ${formatBytes(total)}`,
          cancelling,
          onCancel: run(cancelDownload),
        };
      }
      case "preparing":
        return { kind: "preparing" };
      case "installed":
        return {
          kind: "installed",
          active: model.active,
          onUse: () => select.mutate(model.id),
          onDelete: run(deleteModel),
        };
      case "failed":
        return {
          kind: "error",
          message: model.state.message,
          onRetry: run(downloadModel),
        };
    }
  }
  const installed = models.data.filter(
    (model) => model.state.kind === "installed",
  );
  const activeMissing =
    !models.data.some((model) => model.state.kind === "preparing") &&
    models.data.some(
      (model) => model.active && model.state.kind !== "installed",
    );

  return (
    <div className="models">
      {whisper.data && <WhisperMissing onCheck={() => whisper.refetch()} />}
      {activeMissing && (
        <p className="models-hint" role="status">
          {installed.length === 0
            ? "Download a model to start dictating. Small English is a good first choice."
            : "Your chosen model isn’t downloaded. Use one of yours, or download it again."}
        </p>
      )}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {models.data.map((model) => (
        <ModelCard
          key={model.id}
          name={model.name}
          description={model.description}
          size={formatBytes(model.bytes)}
          badge={model.recommended ? "Recommended" : undefined}
          disabled={busy}
          ratings={[
            {
              label: "Accuracy",
              value: model.accuracy / 5,
              text: accuracyLabel(model.accuracy),
            },
            {
              label: "Speed",
              value: fastest / model.measuredMs,
              text: speedLabel(model.measuredMs),
            },
          ]}
          status={status(model)}
        />
      ))}
      <p className="ui-date models-footnote">
        Speed is the median time to transcribe a{" "}
        {MEASURED_CLIP_SECONDS.toLocaleString()} s sentence, measured on an
        Apple M4 Pro. Accuracy is an estimate from model size, not measured
        here. Models are saved in dictate’s app data folder.
      </p>
    </div>
  );
}

function WhisperMissing({ onCheck }: { onCheck: () => void }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="ui-toast ui-toast-danger models-whisper" role="alert">
      <span aria-hidden="true">
        <CircleAlert size={16} />
      </span>
      <div className="models-whisper-body">
        <p className="models-whisper-title">
          whisper-cli isn’t installed, so dictate can’t transcribe.
        </p>
        <p>Install it from Terminal, then check again:</p>
        <code>{WHISPER_INSTALL}</code>
        <div className="ui-actions">
          <Button
            size="sm"
            onClick={() => {
              void writeText(WHISPER_INSTALL)
                .then(() => setCopied(true))
                .catch(console.error);
            }}
          >
            {copied ? "Copied" : "Copy command"}
          </Button>
          <Button size="sm" onClick={onCheck}>
            Check again
          </Button>
        </div>
      </div>
    </div>
  );
}
