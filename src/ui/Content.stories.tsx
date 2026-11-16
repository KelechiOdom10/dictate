import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";
import { VStack } from "@astryxdesign/core/VStack";
import { ModelCard } from "./Content";

const meta = { title: "Components/Content" } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
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
    <VStack gap={4}>
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
    </VStack>
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

export const ModelDelete: Story = {
  name: "ModelCard / delete asks first",
  render: () => (
    <ModelCard
      name="Tiny English"
      description="Instant, for short notes. Misses more words."
      ratings={ratings(0.2, "Basic", 1, "190 ms")}
      size="78 MB"
      status={{
        kind: "installed",
        active: false,
        onUse: () => {},
        onDelete: deleteModel,
      }}
    />
  ),
  play: async ({ canvasElement }) => {
    deleteModel.mockClear();
    await userEvent.click(
      within(canvasElement).getByRole("button", {
        name: "Delete Tiny English",
      }),
    );
    const dialog = within(document.body).getByRole("alertdialog", {
      name: "Delete Tiny English?",
    });
    await userEvent.click(
      within(dialog).getByRole("button", { name: "Delete model" }),
    );
    await expect(deleteModel).toHaveBeenCalledOnce();
  },
};
const deleteModel = fn();
