import { useEffect, useRef, useState } from "react";
import { Download, Trash2 } from "lucide-react";
import { AlertDialog } from "@astryxdesign/core/AlertDialog";
import { Badge } from "@astryxdesign/core/Badge";
import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
import { Heading } from "@astryxdesign/core/Heading";
import { HStack } from "@astryxdesign/core/HStack";
import { Link } from "@astryxdesign/core/Link";
import {
  MetadataList,
  MetadataListItem,
} from "@astryxdesign/core/MetadataList";
import { ProgressBar } from "@astryxdesign/core/ProgressBar";
import { Text } from "@astryxdesign/core/Text";
import { VStack } from "@astryxdesign/core/VStack";

export type ModelRating = {
  label: string;
  /** 0–1, drawn as a meter. */
  value: number;
  /** What the value means in words, e.g. "Good" or "0.4 s". */
  text: string;
};
export type ModelStatus =
  | { kind: "available"; onDownload: () => void }
  | {
      kind: "downloading";
      progress: number;
      detail: string;
      cancelling: boolean;
      onCancel: () => void;
    }
  | { kind: "preparing" }
  | {
      kind: "installed";
      active: boolean;
      onUse: () => void;
      onDelete: () => void;
    }
  | { kind: "error"; message: string; onRetry: () => void };
export function ModelCard({
  name,
  description,
  ratings,
  size,
  badge,
  licence,
  status,
  disabled = false,
}: {
  name: string;
  description: string;
  ratings: ModelRating[];
  size: string;
  badge?: string;
  /** Terms the weights are published under, opened in the browser. */
  licence?: { name: string; onOpen: () => void };
  status: ModelStatus;
  disabled?: boolean;
}) {
  const [confirming, setConfirming] = useState(false);
  const active = status.kind === "installed" && status.active;
  const card = useRef<HTMLElement>(null);
  const hadFocus = useRef(false);
  // Use, Cancel and Delete each remove the button that was pressed. Keep
  // keyboard focus in this card instead of letting it fall to the page.
  useEffect(() => {
    const focused = document.activeElement;
    if (
      !hadFocus.current ||
      (focused !== document.body && focused !== card.current)
    )
      return;
    const next =
      card.current?.querySelector<HTMLElement>(
        "button:not([aria-disabled='true'])",
      ) ?? card.current;
    next?.focus();
  }, [status.kind, active, confirming]);
  const actions = (() => {
    switch (status.kind) {
      case "available":
        return (
          <Button
            isDisabled={disabled}
            label={`Download ${name}`}
            icon={<Download size={14} aria-hidden="true" />}
            onClick={status.onDownload}
          >
            Download
          </Button>
        );
      case "downloading":
        return (
          <>
            <ProgressBar
              label={status.cancelling ? "Cancelling" : "Downloading"}
              value={status.progress}
              hasValueLabel
              formatValueLabel={() => status.detail}
            />
            <Button
              size="sm"
              variant="ghost"
              isDisabled={status.cancelling}
              label={`Cancel ${name} download`}
              onClick={status.onCancel}
            >
              Cancel
            </Button>
          </>
        );
      case "preparing":
        return (
          <Text role="status" type="supporting">
            Preparing your model…
          </Text>
        );
      case "installed":
        return status.active ? (
          <Text type="supporting">Used for every dictation</Text>
        ) : (
          <>
            <Button
              size="sm"
              variant="ghost"
              isDisabled={disabled}
              label={`Delete ${name}`}
              icon={<Trash2 size={14} aria-hidden="true" />}
              onClick={() => setConfirming(true)}
            >
              Delete
            </Button>
            <Button
              variant="primary"
              isDisabled={disabled}
              label={`Use ${name}`}
              onClick={status.onUse}
            >
              Use
            </Button>
          </>
        );
      case "error":
        return (
          <>
            <Banner status="error" title={status.message} collapsible={false} />
            <Button
              isDisabled={disabled}
              label={`Retry ${name} download`}
              onClick={status.onRetry}
            >
              Retry
            </Button>
          </>
        );
    }
  })();
  return (
    <section
      ref={card}
      tabIndex={-1}
      aria-label={name}
      aria-current={active ? "true" : undefined}
      onFocus={() => (hadFocus.current = true)}
      onBlur={(e) => {
        if (e.relatedTarget)
          hadFocus.current = card.current?.contains(e.relatedTarget) ?? false;
      }}
    >
      <Card variant={active ? "muted" : "default"}>
        <VStack gap={4}>
          <VStack gap={3}>
            <VStack gap={1}>
              <HStack gap={2} vAlign="center">
                <Heading level={2}>{name}</Heading>
                {active && <Badge variant="success" label="In use" />}
                {badge && <Badge label={badge} />}
              </HStack>
              <Text type="supporting">{description}</Text>
              {licence && (
                <Link
                  as="button"
                  isExternalLink
                  hasUnderline
                  onClick={licence.onOpen}
                  type="supporting"
                >
                  {licence.name}
                </Link>
              )}
            </VStack>
            <MetadataList columns={3}>
              {ratings.map((rating) => (
                <MetadataListItem key={rating.label} label={rating.label}>
                  <HStack gap={2} vAlign="center">
                    <meter
                      className="ui-rating"
                      min={0}
                      max={1}
                      value={rating.value}
                      aria-label={`${name} ${rating.label.toLowerCase()}`}
                      aria-valuetext={rating.text}
                    />
                    <Text>{rating.text}</Text>
                  </HStack>
                </MetadataListItem>
              ))}
              <MetadataListItem label="Size">
                <Text>{size}</Text>
              </MetadataListItem>
            </MetadataList>
          </VStack>
          <HStack gap={2} hAlign="end" vAlign="center">
            {actions}
          </HStack>
        </VStack>
      </Card>
      {status.kind === "installed" && !status.active && (
        <AlertDialog
          isOpen={confirming}
          onOpenChange={setConfirming}
          title={`Delete ${name}?`}
          description={`Frees ${size}. You can download it again later.`}
          actionLabel="Delete model"
          onAction={() => {
            setConfirming(false);
            status.onDelete();
          }}
        />
      )}
    </section>
  );
}
