import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { Code } from "@astryxdesign/core/Code";
import { HStack } from "@astryxdesign/core/HStack";
import { Text } from "@astryxdesign/core/Text";
import { VStack } from "@astryxdesign/core/VStack";
import {
  accuracyLabel,
  cancelDownload,
  deleteModel,
  downloadModel,
  formatBytes,
  MEASURED_CLIP_SECONDS,
  openModelLicence,
  readModels,
  readWhisperStatus,
  speedLabel,
  WHISPER_INSTALL,
  type ModelEntry,
  type SpeechModel,
} from "./models";
import { readSettings, writeSettings } from "./settings";
import { ModelCard, type ModelStatus } from "./ui";
import { ErrorNotice, LoadError, Loading } from "./SettingsStatus";

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
    // The chosen model decides whether whisper-cli is needed at all.
    const subscriptions = [
      listen("models-changed", () =>
        client.invalidateQueries({ queryKey: ["models"] }),
      ),
      listen("settings-changed", () => {
        void client.invalidateQueries({ queryKey: ["models"] });
        void client.invalidateQueries({ queryKey: ["whisper"] });
      }),
    ];
    return () => {
      for (const subscription of subscriptions)
        void subscription.then((stop) => stop()).catch(console.error);
    };
  }, [client]);

  if (models.isPending) return <Loading>Loading models…</Loading>;
  if (models.isError)
    return (
      <LoadError error={models.error} onRetry={() => void models.refetch()} />
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
  const recommended = models.data.find((model) => model.recommended);
  const activeMissing =
    !models.data.some((model) => model.state.kind === "preparing") &&
    models.data.some(
      (model) => model.active && model.state.kind !== "installed",
    );

  return (
    <VStack gap={4}>
      {whisper.data && <WhisperMissing onCheck={() => whisper.refetch()} />}
      {activeMissing && (
        <Banner
          role="status"
          status="warning"
          collapsible={false}
          title={
            installed.length === 0
              ? `Download a model to start dictating. ${recommended?.name ?? "The recommended model"} is a good first choice.`
              : "Your chosen model isn’t downloaded. Use one of yours, or download it again."
          }
        />
      )}
      <ErrorNotice error={error} />
      {[
        ...models.data.filter((model) => model.recommended),
        ...models.data.filter((model) => !model.recommended),
      ].map((model) => (
        <ModelCard
          key={model.id}
          name={model.name}
          description={model.description}
          size={formatBytes(model.bytes)}
          badge={model.recommended ? "Recommended" : undefined}
          licence={
            model.licence
              ? {
                  name: model.licence.name,
                  onOpen: () =>
                    void openModelLicence(model.id).catch((e) =>
                      setError(String(e)),
                    ),
                }
              : undefined
          }
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
      <Text type="supporting">
        Speed is the median time to transcribe a{" "}
        {MEASURED_CLIP_SECONDS.toLocaleString()} s sentence, measured on an
        Apple M4 Pro. Accuracy is an estimate, not measured here. Models are
        saved in dictate’s app data folder.
      </Text>
    </VStack>
  );
}

function WhisperMissing({ onCheck }: { onCheck: () => void }) {
  const [copied, setCopied] = useState(false);
  return (
    <Banner
      status="error"
      title="whisper-cli isn’t installed, so dictate can’t transcribe."
      description="Install it from Terminal, then check again."
      collapsible={false}
    >
      <VStack gap={2}>
        <Code>{WHISPER_INSTALL}</Code>
        <HStack gap={2}>
          <Button
            size="sm"
            label={copied ? "Copied" : "Copy command"}
            onClick={() => {
              void writeText(WHISPER_INSTALL)
                .then(() => setCopied(true))
                .catch(console.error);
            }}
          />
          <Button size="sm" label="Check again" onClick={onCheck} />
        </HStack>
      </VStack>
    </Banner>
  );
}
