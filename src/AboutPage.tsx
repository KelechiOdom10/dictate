import { useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useMutation, useQuery } from "@tanstack/react-query";
import { ExternalLink, FolderOpen, Info } from "lucide-react";
import { openLocation, readAppFolders, type Location } from "./about";
import { Button, PathText, SettingsGroup, SettingsRow as Row } from "./ui";

export default function AboutPage() {
  const version = useQuery({ queryKey: ["version"], queryFn: getVersion });
  const folders = useQuery({
    queryKey: ["app-folders"],
    queryFn: readAppFolders,
  });
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
      id={location}
      size="sm"
      aria-label={`Show in Finder, ${name}`}
      aria-describedby={`${location}-help`}
      disabled={!path}
      onClick={() => open.mutate(location)}
    >
      <FolderOpen size={14} aria-hidden="true" /> Show in Finder
    </Button>
  );
  return (
    <>
      <SettingsGroup
        title="dictate"
        icon={<Info size={14} aria-hidden="true" />}
      >
        <Row
          id="version"
          label="Version"
          help="The dictate build running now. Include it when you report a problem."
        >
          <output
            id="version"
            className="ui-row-value"
            aria-describedby="version-help"
          >
            {version.data ?? "…"}
          </output>
        </Row>
        <Row
          id="source"
          label="Source code"
          help="Read the code, or follow along as dictate is built."
        >
          <Button
            id="source"
            size="sm"
            aria-label="View on GitHub, source code"
            aria-describedby="source-help"
            onClick={() => open.mutate("source")}
          >
            View on GitHub <ExternalLink size={14} aria-hidden="true" />
          </Button>
        </Row>
      </SettingsGroup>
      <SettingsGroup
        title="Files"
        icon={<FolderOpen size={14} aria-hidden="true" />}
      >
        <Row
          id="data"
          label="App data"
          help={
            <>
              Settings, history, models and debug recordings.
              {folders.data && <PathText>{folders.data.data}</PathText>}
            </>
          }
        >
          {folder("data", "app data", folders.data?.data)}
        </Row>
        <Row
          id="logs"
          label="Logs"
          help={
            <>
              What dictate did and any errors, useful when reporting a problem.
              {folders.data && <PathText>{folders.data.logs}</PathText>}
            </>
          }
        >
          {folder("logs", "logs", folders.data?.logs)}
        </Row>
      </SettingsGroup>
      {(error || folders.isError) && (
        <p className="error" role="alert">
          {error || String(folders.error)}
        </p>
      )}
    </>
  );
}
