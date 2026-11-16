import { useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useMutation, useQuery } from "@tanstack/react-query";
import { ExternalLink, FolderOpen, Info, RefreshCw } from "lucide-react";
import { Button } from "@astryxdesign/core/Button";
import { Text } from "@astryxdesign/core/Text";
import { VStack } from "@astryxdesign/core/VStack";
import { openLocation, readAppFolders, type Location } from "./about";
import { ErrorNotice } from "./SettingsStatus";
import { isUpdateBusy, updateMessage, useUpdates } from "./update";
import { PathText, SettingsGroup, SettingsRow as Row } from "./ui";

export default function AboutPage() {
  const version = useQuery({ queryKey: ["version"], queryFn: getVersion });
  const folders = useQuery({
    queryKey: ["app-folders"],
    queryFn: readAppFolders,
  });
  const updates = useUpdates();
  const [error, setError] = useState("");
  const open = useMutation({
    mutationFn: openLocation,
    onMutate: () => setError(""),
    onError: (e) => setError(String(e)),
  });
  const folder = (
    location: Location,
    name: string,
    path: string | undefined,
  ) => (
    <Button
      size="sm"
      label={`Show in Finder, ${name}`}
      icon={<FolderOpen size={14} aria-hidden="true" />}
      isDisabled={!path}
      onClick={() => open.mutate(location)}
    >
      Show in Finder
    </Button>
  );
  const help = (text: string, path: string | undefined) => (
    <VStack gap={1}>
      <Text type="supporting">{text}</Text>
      {path && <PathText>{path}</PathText>}
    </VStack>
  );
  return (
    <>
      <SettingsGroup
        title="dictate"
        icon={<Info size={14} aria-hidden="true" />}
      >
        <Row
          label="Version"
          help="The dictate build running now. Include it when you report a problem."
        >
          <Text role="status" aria-label="Version" hasTabularNumbers>
            {version.data ?? "…"}
          </Text>
        </Row>
        <Row
          label="Updates"
          help={
            updateMessage(updates.state) ||
            "dictate checks for a new version when it starts, and installs it when you aren't dictating."
          }
        >
          <Button
            size="sm"
            label="Check now"
            icon={<RefreshCw size={14} aria-hidden="true" />}
            isDisabled={isUpdateBusy(updates.state)}
            onClick={() => void updates.check()}
          >
            Check now
          </Button>
        </Row>
        <Row
          label="Source code"
          help="Read the code, or follow along as dictate is built."
        >
          <Button
            size="sm"
            label="View on GitHub, source code"
            endContent={undefined}
            icon={<ExternalLink size={14} aria-hidden="true" />}
            onClick={() => open.mutate("source")}
          >
            View on GitHub
          </Button>
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Files"
        icon={<FolderOpen size={14} aria-hidden="true" />}
      >
        <Row
          label="App data"
          help={help(
            "Settings, history, models and debug recordings.",
            folders.data?.data,
          )}
        >
          {folder("data", "app data", folders.data?.data)}
        </Row>
        <Row
          label="Logs"
          help={help(
            "What dictate did and any errors, useful when reporting a problem.",
            folders.data?.logs,
          )}
        >
          {folder("logs", "logs", folders.data?.logs)}
        </Row>
      </SettingsGroup>
      <ErrorNotice
        error={error || (folders.isError ? String(folders.error) : "")}
      />
    </>
  );
}
