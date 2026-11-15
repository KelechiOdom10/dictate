import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { Copy, Trash2 } from "lucide-react";
import { expect, userEvent, within } from "storybook/test";
import { Button, IconButton } from "./Button";
import {
  Card,
  ListItem,
  DetailPanel,
  ModelCard,
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
export const SelectableNote: Story = {
  name: "ListItem / selectable",
  render: () => {
    const [selected, setSelected] = useState(1);
    const rows = [
      {
        title: "Pick up coffee on the way back",
        meta: "Notes",
        date: "1h ago",
      },
      {
        title: "Ship the history page once the screenshots look right",
        meta: "Slack",
        date: "1d ago",
      },
      { title: "No speech detected", meta: "Notes · Skipped", date: "4d ago" },
    ];
    return (
      <div className="story-stack" style={{ maxWidth: 280 }}>
        {rows.map((row, index) => (
          <ListItem
            key={row.title}
            {...row}
            selected={selected === index}
            onSelect={() => setSelected(index)}
          />
        ))}
      </div>
    );
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const first = c.getByRole("button", { name: /Pick up coffee/ });
    await userEvent.click(first);
    await expect(first).toHaveAttribute("aria-current", "true");
    await expect(
      c.getByRole("button", { name: /Ship the history page/ }),
    ).not.toHaveAttribute("aria-current");
  },
};
export const Detail: Story = {
  name: "DetailPanel",
  render: () => (
    <DetailPanel
      title="1h ago"
      subtitle="14 Nov 2026, 2:57 PM"
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
export const Models: Story = {
  name: "ModelCard / available, downloading, installed, error",
  render: () => (
    <div className="story-stack">
      <ModelCard
        name="Base English"
        description="A balanced everyday companion."
        accuracy={75}
        speed={90}
        size="142 MB"
        badge="Recommended"
        status={{ kind: "available", onDownload: () => {} }}
      />
      <ModelCard
        name="Small English"
        description="More detail, a little more time."
        accuracy={90}
        speed={65}
        size="466 MB"
        status={{ kind: "downloading", progress: 42 }}
      />
      <ModelCard
        name="Tiny English"
        description="For quick thoughts."
        accuracy={60}
        speed={100}
        size="75 MB"
        status={{ kind: "installed" }}
      />
      <ModelCard
        name="Medium English"
        description="For careful transcription."
        accuracy={95}
        speed={40}
        size="1.5 GB"
        status={{
          kind: "error",
          message: "Download interrupted. Try again.",
          onRetry: () => {},
        }}
      />
    </div>
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
