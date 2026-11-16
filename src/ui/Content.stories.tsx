import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Copy, Trash2 } from "lucide-react";
import { expect, fn, userEvent, within } from "storybook/test";
import { Button, IconButton } from "./Button";
import {
  Card,
  ListItem,
  DetailPanel,
  ModelCard,
  SelectList,
  ProgressBar,
  EmptyState,
  Toast,
} from "./Content";
const meta = { title: "Components/Content" } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
export const Surface: Story = {
  name: "Card",
  render: () => (
    <Card>
      <h3>A little room to think</h3>
      <p className="ui-preview">Your voice stays on this device.</p>
    </Card>
  ),
};
export const Note: Story = {
  name: "ListItem / with and without actions",
  render: () => (
    <div>
      <ListItem
        date="Today · 9:41 AM"
        title="A thought for later"
        preview="Make space for the things that matter."
        actions={<Button variant="ghost">Copy text</Button>}
      />
      <ListItem
        date="Yesterday · 4:30 PM"
        title="A quiet moment"
        meta="Notes"
        preview="Take the long way home."
      />
    </div>
  ),
};
export const Selectable: Story = {
  name: "SelectList / pointer and arrow keys",
  render: () => {
    const [selected, setSelected] = useState(2);
    return (
      <div style={{ maxWidth: 280 }}>
        <SelectList
          label="Dictations"
          items={[
            {
              id: 1,
              title: "Pick up coffee on the way back",
              meta: "Notes",
              date: "1h ago",
            },
            {
              id: 2,
              title: "Ship the history page once the screenshots look right",
              meta: "Slack",
              date: "1d ago",
            },
            {
              id: 3,
              title: "No speech detected",
              meta: "Notes · Skipped",
              date: "4d ago",
            },
          ]}
          value={selected}
          onChange={setSelected}
        />
      </div>
    );
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const first = c.getByRole("button", { name: /Pick up coffee/ });
    const second = c.getByRole("button", { name: /Ship the history page/ });
    const third = c.getByRole("button", { name: /No speech detected/ });
    await expect(second).toHaveAttribute("tabindex", "0");
    await expect(first).toHaveAttribute("tabindex", "-1");
    await userEvent.click(first);
    await expect(first).toHaveAttribute("aria-current", "true");
    await userEvent.keyboard("{End}");
    await expect(third).toHaveFocus();
    await expect(third).toHaveAttribute("aria-current", "true");
    await userEvent.keyboard("{ArrowUp}");
    await expect(second).toHaveFocus();
  },
};
export const Detail: Story = {
  name: "DetailPanel",
  render: () => (
    <DetailPanel
      title="Dictation"
      subtitle="1h ago · 14 Nov 2026, 2:57 PM"
      actions={
        <>
          <IconButton variant="ghost" label="Copy text">
            <Copy size={16} aria-hidden="true" />
          </IconButton>
          <IconButton variant="ghost" label="Delete dictation">
            <Trash2 size={16} aria-hidden="true" />
          </IconButton>
        </>
      }
      metadata={[
        { label: "Application", value: "Notes" },
        { label: "Speech model", value: "Whisper base.en" },
        { label: "Latency", value: "4.2 s audio · 310 ms transcription" },
      ]}
    >
      <p>Make space for the things that matter. Start with one small thing.</p>
    </DetailPanel>
  ),
};
const ratings = (
  accuracy: number,
  accuracyText: string,
  speed: number,
  speedText: string,
) => [
  { label: "Accuracy", value: accuracy, text: accuracyText },
  { label: "Speed", value: speed, text: speedText },
];
export const Models: Story = {
  name: "ModelCard / available, downloading, installed, in use, error",
  render: () => (
    <div className="story-stack">
      <ModelCard
        name="Small English"
        description="Noticeably more accurate, still quick on this Mac."
        ratings={ratings(0.6, "Good", 0.43, "440 ms")}
        size="488 MB"
        badge="Recommended"
        status={{ kind: "available", onDownload: () => {} }}
      />
      <ModelCard
        name="Medium English"
        description="Careful with names and jargon, but slower."
        ratings={ratings(0.8, "Very good", 0.18, "1.0 s")}
        size="1.5 GB"
        status={{
          kind: "downloading",
          progress: 42,
          detail: "644 MB of 1.5 GB",
          cancelling: false,
          onCancel: () => {},
        }}
      />
      <ModelCard
        name="Tiny English"
        description="Instant, for short notes. Misses more words."
        ratings={ratings(0.2, "Basic", 1, "190 ms")}
        size="78 MB"
        status={{
          kind: "installed",
          active: false,
          onUse: () => {},
          onDelete: () => {},
        }}
      />
      <ModelCard
        name="Base English"
        description="Fast and dependable for everyday dictation."
        ratings={ratings(0.4, "Fair", 0.81, "190 ms")}
        size="148 MB"
        status={{
          kind: "installed",
          active: true,
          onUse: () => {},
          onDelete: () => {},
        }}
      />
      <ModelCard
        name="Large v3 Turbo"
        description="The most accurate. Multilingual, set to English here."
        ratings={ratings(1, "Best", 0.17, "1.1 s")}
        size="1.6 GB"
        status={{
          kind: "error",
          message: "Download failed. Check your connection and try again.",
          onRetry: () => {},
        }}
      />
    </div>
  ),
};
const openLicence = fn();
export const ModelLicence: Story = {
  name: "ModelCard / with the model's licence",
  render: () => (
    <ModelCard
      name="Parakeet English"
      description="NVIDIA's dictation model. Very accurate, and stays loaded so text is ready fast."
      ratings={ratings(1, "Best", 1, "130 ms")}
      size="731 MB"
      licence={{ name: "NVIDIA Open Model License", onOpen: openLicence }}
      status={{ kind: "available", onDownload: () => {} }}
    />
  ),
  play: async ({ canvasElement }) => {
    openLicence.mockClear();
    await userEvent.click(
      within(canvasElement).getByRole("button", {
        name: "NVIDIA Open Model License",
      }),
    );
    await expect(openLicence).toHaveBeenCalledOnce();
  },
};
export const ModelPreparing: Story = {
  name: "ModelCard / preparing a model from an earlier version",
  render: () => (
    <ModelCard
      name="Base English"
      description="Fast and dependable for everyday dictation."
      ratings={ratings(0.4, "Fair", 0.81, "190 ms")}
      size="148 MB"
      status={{ kind: "preparing" }}
    />
  ),
};
export const Progress: Story = {
  name: "ProgressBar / empty, partial, complete, indeterminate",
  render: () => (
    <div className="story-stack">
      <ProgressBar label="Starting" value={0} />
      <ProgressBar label="Downloading" value={42} />
      <ProgressBar label="Complete" value={100} />
      <ProgressBar label="Preparing" />
    </div>
  ),
};
export const Empty: Story = {
  name: "EmptyState / with and without action",
  render: () => (
    <div>
      <EmptyState
        title="Your words will live here"
        action={<Button variant="primary">Start dictating</Button>}
      >
        Once you dictate, your recent text appears here.
      </EmptyState>
      <EmptyState title="No results">Try another search.</EmptyState>
    </div>
  ),
};
export const Toasts: Story = {
  name: "Toast / success, warning, error",
  render: () => (
    <div className="story-stack">
      <Toast onDismiss={() => {}}>Your preferences are up to date.</Toast>
      <Toast tone="warning">Your microphone is disconnected.</Toast>
      <Toast tone="danger" onDismiss={() => {}}>
        Could not save your shortcut.
      </Toast>
    </div>
  ),
};
