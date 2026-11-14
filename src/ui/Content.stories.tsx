import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "./Button";
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
        preview="Take the long way home."
      />
    </div>
  ),
};
export const Detail: Story = {
  name: "DetailPanel",
  render: () => (
    <DetailPanel
      title="A thought for later"
      metadata={[
        { label: "Recorded", value: "Today, 9:41 AM" },
        { label: "Duration", value: "33 seconds" },
        { label: "Model", value: "Base English" },
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
